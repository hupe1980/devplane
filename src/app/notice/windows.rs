//! Windows: a toast built with the platform's own XML document object, so
//! every piece of text is a text node and never markup.
//!
//! Buttons carry `<key>:<index>` as their arguments, a text box and its send
//! button `<key>:reply`, and the toast's own launch argument `<key>:default`
//! is a click on its body. Each toast is tagged with its key in the `devplane`
//! group so it can be taken back from the action centre. The default scenario
//! and the default sound: Focus assist holds it like any ordinary toast.
//!
//! Activation reaches this process only while it runs: no COM activator is
//! registered, so a toast pressed after the app quit does nothing — and the
//! app takes its toasts back as it quits. A build with no registered app id
//! (run from `target/`) gets the plain fallback.

use super::{NoticeSink, Respond, Response, split_action};
use crate::core::notice::{Capability, Notice, NoticeAnswer};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use windows::Data::Xml::Dom::{XmlDocument, XmlElement};
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
};
use windows::core::{HSTRING, IInspectable, Interface};

/// How many answer actions one toast offers: the schema allows five buttons.
pub const ACTIONS: u8 = 5;
/// The group every Devplane toast is in.
const GROUP: &str = "devplane";
/// The text box's id, for a question that takes the person's own words.
const REPLY_INPUT: &str = "reply";

pub struct WindowsSink {
    app_id: String,
    /// Live toasts, kept so their activation handlers stay registered.
    toasts: Arc<Mutex<HashMap<String, ToastNotification>>>,
}

impl WindowsSink {
    /// The sink for an installed app; `None` for a build run from `target/`,
    /// which has no registered app id for activation to reach.
    pub fn new(app_id: &str) -> Option<WindowsSink> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?.to_string_lossy().to_string();
        let sep = std::path::MAIN_SEPARATOR;
        if dir.ends_with(&format!("{sep}target{sep}debug"))
            || dir.ends_with(&format!("{sep}target{sep}release"))
        {
            return None;
        }
        Some(WindowsSink {
            app_id: app_id.to_string(),
            toasts: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

/// The toast's tag: its key, cut to the 64 characters a tag may hold.
fn tag_of(key: &str) -> String {
    key.chars().take(64).collect()
}

fn element(doc: &XmlDocument, name: &str) -> windows::core::Result<XmlElement> {
    doc.CreateElement(&HSTRING::from(name))
}

fn attr(e: &XmlElement, name: &str, value: &str) -> windows::core::Result<()> {
    e.SetAttribute(&HSTRING::from(name), &HSTRING::from(value))
}

/// A `<text>` element holding `text` as a text node.
fn text(doc: &XmlDocument, text: &str) -> windows::core::Result<XmlElement> {
    let e = element(doc, "text")?;
    e.AppendChild(&doc.CreateTextNode(&HSTRING::from(text))?)?;
    Ok(e)
}

/// The toast document for one notice.
pub fn document(notice: &Notice) -> windows::core::Result<XmlDocument> {
    let doc = XmlDocument::new()?;
    let toast = element(&doc, "toast")?;
    attr(&toast, "launch", &format!("{}:default", notice.key))?;
    doc.AppendChild(&toast)?;

    let visual = element(&doc, "visual")?;
    let binding = element(&doc, "binding")?;
    attr(&binding, "template", "ToastGeneric")?;
    binding.AppendChild(&text(&doc, &notice.title)?)?;
    binding.AppendChild(&text(&doc, &notice.body)?)?;
    visual.AppendChild(&binding)?;
    toast.AppendChild(&visual)?;

    if !notice.actions.is_empty() {
        let actions = element(&doc, "actions")?;
        let reply = notice
            .actions
            .iter()
            .any(|a| matches!(a.answers, NoticeAnswer::Reply { .. }));
        if reply {
            let input = element(&doc, "input")?;
            attr(&input, "id", REPLY_INPUT)?;
            attr(&input, "type", "text")?;
            attr(&input, "placeHolderContent", "Your answer")?;
            actions.AppendChild(&input)?;
        }
        for a in &notice.actions {
            let action = element(&doc, "action")?;
            attr(&action, "content", &a.label)?;
            match a.answers {
                NoticeAnswer::Reply { .. } => {
                    attr(&action, "arguments", &format!("{}:reply", notice.key))?;
                    attr(&action, "hint-inputId", REPLY_INPUT)?;
                }
                _ => attr(&action, "arguments", &format!("{}:{}", notice.key, a.index))?,
            }
            actions.AppendChild(&action)?;
        }
        toast.AppendChild(&actions)?;
    }
    Ok(doc)
}

/// What the person did, from an activation's arguments and typed text.
fn response_of(arguments: &str, typed: Option<String>) -> Response {
    if arguments.ends_with(":default") {
        return Response::Clicked;
    }
    if arguments.ends_with(":reply") {
        return match typed {
            Some(t) => Response::Replied(t),
            None => Response::Dismissed,
        };
    }
    match split_action(arguments) {
        Some((_, i)) => Response::Pressed(i),
        None => Response::Dismissed,
    }
}

/// The key an activation's arguments name.
fn key_of(arguments: &str) -> String {
    arguments
        .rsplit_once(':')
        .map(|(k, _)| k.to_string())
        .unwrap_or_default()
}

/// The text typed into the reply box, if any.
fn typed(args: &ToastActivatedEventArgs) -> Option<String> {
    let inputs = args.UserInput().ok()?;
    let value = inputs.Lookup(&HSTRING::from(REPLY_INPUT)).ok()?;
    let value: windows::Foundation::IReference<HSTRING> = value.cast().ok()?;
    value.Value().ok().map(|s| s.to_string())
}

impl WindowsSink {
    fn show(&self, notice: &Notice, respond: Respond) -> windows::core::Result<()> {
        let toast = ToastNotification::CreateToastNotification(&document(notice)?)?;
        let tag = tag_of(&notice.key);
        toast.SetTag(&HSTRING::from(tag.as_str()))?;
        toast.SetGroup(&HSTRING::from(GROUP))?;
        let toasts = self.toasts.clone();
        toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_, args| {
                let Some(args) = args.as_ref() else {
                    return Ok(());
                };
                let args: ToastActivatedEventArgs = args.cast()?;
                let arguments = args.Arguments()?.to_string();
                let key = key_of(&arguments);
                if let Ok(mut t) = toasts.lock() {
                    t.remove(&key);
                }
                respond(key, response_of(&arguments, typed(&args)));
                Ok(())
            },
        ))?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(&self.app_id))?
            .Show(&toast)?;
        if let Ok(mut t) = self.toasts.lock() {
            t.insert(notice.key.clone(), toast);
        }
        Ok(())
    }
}

impl NoticeSink for WindowsSink {
    fn capability(&self) -> Capability {
        Capability {
            actions: ACTIONS,
            reply: true,
            withdraw: true,
            click: true,
        }
    }

    fn deliver(&self, notice: &Notice, respond: Respond) -> Result<(), String> {
        self.show(notice, respond).map_err(|e| e.to_string())
    }

    fn withdraw(&self, key: &str) {
        if let Ok(mut t) = self.toasts.lock() {
            t.remove(key);
        }
        let removed = ToastNotificationManager::History().and_then(|h| {
            h.RemoveGroupedTagWithId(
                &HSTRING::from(tag_of(key).as_str()),
                &HSTRING::from(GROUP),
                &HSTRING::from(&self.app_id),
            )
        });
        if let Err(e) = removed {
            tracing::debug!(error = %e, "could not take a toast back");
        }
    }

    fn describe(&self) -> String {
        "actions and reply (Windows, installed app): a permission's Allow and Deny, a \
         question's options up to five, or a typed reply"
            .into()
    }
}

//! Desktop notifications.
//!
//! Shelling out to the platform's own notifier rather than linking a crate.
//! On macOS a notification needs a bundle identifier, and a plain CLI binary
//! has none — the usual Rust crate silently attributes the notification to
//! Finder. `osascript` is already how windows are raised, works from a daemon,
//! and adds no dependency.
//!
//! The discipline matters more than the mechanism. A notification is an
//! interruption, so only an item that genuinely needs a person earns one, and
//! each one is sent at most once: an inbox that buzzes twice for the same
//! question teaches people to ignore it.

use crate::core::attention::{AttentionItem, Level};
use std::collections::HashSet;
use std::process::{Command, Stdio};

/// Remembers what has already been announced.
#[derive(Debug, Default)]
pub struct Notifier {
    announced: HashSet<String>,
    enabled: bool,
}

impl Notifier {
    pub fn new(enabled: bool) -> Self {
        Self {
            announced: HashSet::new(),
            enabled,
        }
    }

    /// Announces anything in the inbox that is new and urgent enough.
    ///
    /// Items that have gone away are forgotten, so the same question asked
    /// again after being answered does notify again — that is a new decision,
    /// not a repeat.
    pub fn sync(&mut self, inbox: &[AttentionItem]) {
        let current: HashSet<String> = inbox.iter().map(|i| i.id.0.clone()).collect();
        self.announced.retain(|id| current.contains(id));

        if !self.enabled {
            return;
        }
        for item in inbox {
            if item.level < Level::High || self.announced.contains(&item.id.0) {
                continue;
            }
            self.announced.insert(item.id.0.clone());
            send(&title_for(item), &item.title);
        }
    }
}

fn title_for(item: &AttentionItem) -> String {
    match &item.project_id {
        Some(p) => {
            let name = std::path::Path::new(p.as_str())
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string());
            format!("Vibeplane · {name}")
        }
        None => "Vibeplane".to_string(),
    }
}

/// Fires and forgets. A notifier that is missing, or a desktop that is locked,
/// must never become an error in a daemon whose job is to watch quietly.
pub fn send(title: &str, body: &str) {
    let _ = platform_send(title, body);
}

#[cfg(target_os = "macos")]
fn platform_send(title: &str, body: &str) -> std::io::Result<()> {
    // Both strings reach AppleScript as source, so quotes and backslashes are
    // escaped rather than interpolated raw: a tool name with a quote in it must
    // not be able to run script of its own.
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        applescript_escape(body),
        applescript_escape(title)
    );
    Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "macos")]
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "linux")]
fn platform_send(title: &str, body: &str) -> std::io::Result<()> {
    Command::new("notify-send")
        .args(["--app-name=Vibeplane", title, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "windows")]
fn platform_send(title: &str, body: &str) -> std::io::Result<()> {
    let script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType=WindowsRuntime] > $null; \
         $t=[Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent(1); \
         $t.GetElementsByTagName('text')[0].AppendChild($t.CreateTextNode('{}')) > $null; \
         $t.GetElementsByTagName('text')[1].AppendChild($t.CreateTextNode('{}')) > $null; \
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Vibeplane').Show($t)",
        title.replace('\'', "''"),
        body.replace('\'', "''")
    );
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_send(_title: &str, _body: &str) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::attention::{Action, AttentionKind};
    use crate::core::ids::{AttentionId, RunId};

    fn item(id: &str, level: Level) -> AttentionItem {
        AttentionItem {
            id: AttentionId::new(id),
            kind: AttentionKind::Question,
            level,
            run_id: Some(RunId::new("r")),
            project_id: None,
            title: "Keep the legacy route?".into(),
            detail: None,
            options: vec![],
            actions: vec![Action::Focus],
            request_id: None,
            url: None,
            launch: None,
            work_id: None,
            suggested_rule: None,
            since: jiff::Timestamp::now(),
        }
    }

    #[test]
    fn each_item_is_announced_once() {
        let mut n = Notifier::new(false);
        let inbox = vec![item("a", Level::High)];
        n.sync(&inbox);
        n.sync(&inbox);
        // Disabled, so nothing is sent, but the bookkeeping is what is tested:
        // a second sync must not queue the same item again.
        assert_eq!(n.announced.len(), 0, "nothing is remembered while disabled");

        let mut n = Notifier {
            announced: Default::default(),
            enabled: true,
        };
        n.sync(&inbox);
        assert_eq!(n.announced.len(), 1);
        n.sync(&inbox);
        assert_eq!(n.announced.len(), 1, "a second pass must not re-announce");
    }

    #[test]
    fn resolving_an_item_lets_it_notify_again_later() {
        // The same question asked again after being answered is a new decision.
        let mut n = Notifier {
            announced: Default::default(),
            enabled: true,
        };
        n.sync(&[item("a", Level::High)]);
        n.sync(&[]);
        assert!(n.announced.is_empty(), "a resolved item is forgotten");
        n.sync(&[item("a", Level::High)]);
        assert_eq!(n.announced.len(), 1);
    }

    #[test]
    fn quiet_items_never_interrupt() {
        let mut n = Notifier {
            announced: Default::default(),
            enabled: true,
        };
        n.sync(&[item("a", Level::Normal), item("b", Level::Normal)]);
        assert!(
            n.announced.is_empty(),
            "only high and critical earn an interruption"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn applescript_strings_are_escaped() {
        // A tool name is attacker-influenced text: it must be data in the
        // script, never more script.
        let evil = r#"x" & (do shell script "touch /tmp/pwned") & ""#;
        let escaped = applescript_escape(evil);
        // Every quote in the result must be escaped, so none of them can close
        // the string literal the script embeds it in.
        let bytes = escaped.as_bytes();
        for (i, b) in bytes.iter().enumerate() {
            if *b == b'"' {
                assert!(
                    i > 0 && bytes[i - 1] == b'\\',
                    "unescaped quote at {i} in {escaped}"
                );
            }
        }
        assert!(escaped.contains("\\\""), "and quoting actually happened");

        // A backslash must be escaped first, or it would escape our escape.
        assert_eq!(applescript_escape(r#"a\"b"#), r#"a\\\"b"#);
    }
}

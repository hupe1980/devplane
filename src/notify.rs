//! Desktop notifications, through the platform's own notifier.
//!
//! On macOS a plain CLI binary has no bundle identifier, so notifier crates
//! attribute to Finder; `osascript` works from a background process and adds
//! no dependency. Each item notifies at most once; the dedup is in memory, so
//! a host restart with a question still open buzzes once more.

use crate::core::attention::{AttentionItem, AttentionKind};
use std::collections::HashSet;
use std::process::{Command, Stdio};

/// The four kinds that earn an interruption, and the only four: a question
/// asked, a question abandoned, a permission waiting, and a gate that went red
/// after its agent stopped. A list rather than a level, so raising a kind's
/// inbox level cannot make it buzz.
pub fn kinds_that_notify() -> &'static [AttentionKind] {
    &[
        AttentionKind::Question,
        AttentionKind::QuestionAbandoned,
        AttentionKind::Permission,
        AttentionKind::GateFailed,
    ]
}

/// Where a notification goes: the platform's own notifier, or the app's.
///
/// Chosen once at start. [`kinds_that_notify`] decides what notifies; the sink
/// decides only how it reaches the screen.
pub enum Sink {
    /// `osascript`, `notify-send`, PowerShell: the CLI host's path.
    Os,
    /// The app's notification plugin; the notification is the app's own.
    #[cfg(feature = "app")]
    Tauri(tauri::AppHandle),
}

static SINK: std::sync::OnceLock<Sink> = std::sync::OnceLock::new();

/// Sets the sink for this process, once; a second choice is refused.
pub fn choose(sink: Sink) -> bool {
    SINK.set(sink).is_ok()
}

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
    /// Items that went away are forgotten, so the same question asked again
    /// after being answered notifies again.
    pub fn sync(&mut self, inbox: &[AttentionItem]) {
        let current: HashSet<String> = inbox.iter().map(|i| i.id.0.clone()).collect();
        self.announced.retain(|id| current.contains(id));

        if !self.enabled {
            return;
        }
        for item in inbox {
            if !kinds_that_notify().contains(&item.kind) || self.announced.contains(&item.id.0) {
                continue;
            }
            self.announced.insert(item.id.0.clone());
            announce(project_of(item).as_deref(), &item.title);
        }
    }
}

/// The project's name — the last segment of its path, which is its id.
fn project_of(item: &AttentionItem) -> Option<String> {
    item.project_id.as_ref().map(|p| {
        std::path::Path::new(p.as_str())
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.to_string())
    })
}

/// One inbox item, to whichever sink was chosen.
///
/// The OS notifier names the product in the title; the app's notification
/// already carries the app's name, so its title is the project alone.
fn announce(project: Option<&str>, what: &str) {
    match SINK.get().unwrap_or(&Sink::Os) {
        Sink::Os => send(
            &match project {
                Some(p) => format!("Devplane · {p}"),
                None => "Devplane".to_string(),
            },
            what,
        ),
        #[cfg(feature = "app")]
        Sink::Tauri(app) => {
            use tauri_plugin_notification::NotificationExt;
            if let Err(e) = app
                .notification()
                .builder()
                .title(project.unwrap_or("Devplane"))
                .body(what)
                .show()
            {
                tracing::warn!(error = %e, "the notification was not shown");
            }
        }
    }
}

/// Fires and forgets through the platform's notifier: a missing notifier or a
/// locked desktop is never an error.
pub fn send(title: &str, body: &str) {
    if let Some(mut cmd) = platform_command(title, body) {
        reap(cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn());
    }
}

/// Waits for the notifier on a detached thread, so the child is reaped (the
/// long-lived host would otherwise collect zombies) and `send` never blocks
/// on a locked desktop. No runtime is assumed: hook processes have none.
fn reap(child: std::io::Result<std::process::Child>) {
    if let Ok(mut child) = child {
        std::thread::Builder::new()
            .name("devplane-notify".into())
            .spawn(move || {
                let _ = child.wait();
            })
            .ok();
    }
}

#[cfg(target_os = "macos")]
fn platform_command(title: &str, body: &str) -> Option<Command> {
    // Both strings reach AppleScript as source, so quotes and backslashes are
    // escaped: a tool name must not be able to run script of its own.
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        applescript_escape(body),
        applescript_escape(title)
    );
    let mut c = Command::new("osascript");
    c.arg("-e").arg(script);
    Some(c)
}

#[cfg(target_os = "macos")]
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "linux")]
fn platform_command(title: &str, body: &str) -> Option<Command> {
    Some(notify_send_command(title, body))
}

/// `notify-send`, with `--` so a title or body that starts with `-` stays a
/// positional rather than being read as an option.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn notify_send_command(title: &str, body: &str) -> Command {
    let mut c = Command::new("notify-send");
    c.args(["--app-name=Devplane", "--", title, body]);
    c
}

#[cfg(target_os = "windows")]
fn platform_command(title: &str, body: &str) -> Option<Command> {
    Some(toast_command(title, body))
}

/// The PowerShell toast script. Static: the title and body never become
/// script source. PowerShell treats the typographic quotes U+2018..U+201B as
/// string delimiters too, so no amount of escaping `'` is enough; the text
/// arrives through the environment instead and is only ever read as a value.
const TOAST_SCRIPT: &str = "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType=WindowsRuntime] > $null; \
     $t=[Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent(1); \
     $t.GetElementsByTagName('text')[0].AppendChild($t.CreateTextNode($env:DEVPLANE_TOAST_TITLE)) > $null; \
     $t.GetElementsByTagName('text')[1].AppendChild($t.CreateTextNode($env:DEVPLANE_TOAST_BODY)) > $null; \
     [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Devplane').Show($t)";

/// `powershell` showing a toast, with the text passed as environment
/// variables. Built on every platform so the separation is testable anywhere.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn toast_command(title: &str, body: &str) -> Command {
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-Command", TOAST_SCRIPT])
        .env("DEVPLANE_TOAST_TITLE", title)
        .env("DEVPLANE_TOAST_BODY", body);
    c
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_command(_title: &str, _body: &str) -> Option<Command> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::attention::{Action, Level};
    use crate::core::ids::{AttentionId, RunId};

    fn item(id: &str, kind: AttentionKind) -> AttentionItem {
        AttentionItem {
            id: AttentionId::new(id),
            kind,
            level: kind.default_level(),
            run_id: Some(RunId::new("r")),
            project_id: None,
            title: "Keep the legacy route?".into(),
            detail: None,
            answer_in: None,
            options: vec![],
            actions: vec![Action::Attach],
            ask: None,
            request_id: None,
            form: None,
            url: None,
            launch: None,
            change_id: None,
            offer: None,
            no_offer: None,
            report: None,
            since: jiff::Timestamp::now(),
        }
    }

    #[test]
    fn each_item_is_announced_once() {
        let mut n = Notifier::new(false);
        let inbox = vec![item("a", AttentionKind::Question)];
        n.sync(&inbox);
        n.sync(&inbox);
        // Disabled, so nothing is sent; a second sync must not queue again.
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
        n.sync(&[item("a", AttentionKind::Question)]);
        n.sync(&[]);
        assert!(n.announced.is_empty(), "a resolved item is forgotten");
        n.sync(&[item("a", AttentionKind::Question)]);
        assert_eq!(n.announced.len(), 1);
    }

    /// Four kinds notify, and nothing else does, whatever level the inbox
    /// gives the rest.
    #[test]
    fn exactly_four_kinds_interrupt() {
        assert_eq!(
            kinds_that_notify(),
            &[
                AttentionKind::Question,
                AttentionKind::QuestionAbandoned,
                AttentionKind::Permission,
                AttentionKind::GateFailed,
            ]
        );
        for quiet in [
            AttentionKind::SpecDrifted,
            AttentionKind::Stalled,
            AttentionKind::IssueAssigned,
            AttentionKind::ReviewRequested,
            AttentionKind::PrReady,
            AttentionKind::CiRed,
            AttentionKind::ChangesRequested,
            AttentionKind::Lost,
            AttentionKind::Interrupted,
        ] {
            assert!(
                !kinds_that_notify().contains(&quiet),
                "{quiet:?} must not notify"
            );
        }
        // An abandoned question ranks `Normal` in the inbox and still notifies.
        assert_eq!(
            AttentionKind::QuestionAbandoned.default_level(),
            Level::Normal
        );

        let mut n = Notifier {
            announced: Default::default(),
            enabled: true,
        };
        n.sync(&[
            item("a", AttentionKind::Stalled),
            item("b", AttentionKind::SpecDrifted),
            item("c", AttentionKind::IssueAssigned),
        ]);
        assert!(
            n.announced.is_empty(),
            "only the four kinds earn an interruption"
        );
        n.sync(&[
            item("q", AttentionKind::Question),
            item("x", AttentionKind::QuestionAbandoned),
            item("p", AttentionKind::Permission),
            item("g", AttentionKind::GateFailed),
        ]);
        assert_eq!(n.announced.len(), 4);
    }

    /// The sink is chosen once; a second choice is refused rather than
    /// rerouting notifications already on their way.
    #[test]
    fn the_sink_is_chosen_once() {
        // Whichever test set it first, the second set must fail.
        let _ = choose(Sink::Os);
        assert!(!choose(Sink::Os));
    }

    /// The notifier is reaped on a thread, so `send` returns at once and no
    /// zombie is left behind.
    #[cfg(unix)]
    #[test]
    fn a_spawned_notifier_is_reaped_rather_than_left_a_zombie() {
        let child = Command::new("true")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("`true` runs");
        let pid = child.id();
        reap(Ok(child));
        // Once reaped, the pid is gone (or reused); a zombie stays listed `Z`.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let out = Command::new("ps")
                .args(["-o", "stat=", "-p", &pid.to_string()])
                .output()
                .expect("ps runs");
            let stat = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !stat.starts_with('Z') {
                break;
            }
            if std::time::Instant::now() > deadline {
                panic!("pid {pid} is still `{stat}` five seconds after it exited");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // And a failed spawn is simply nothing to wait for.
        reap(Err(std::io::Error::other("no notifier")));
    }

    /// Agent text reaches the toast only as an environment value: the script
    /// is the same constant whatever the text, so neither ASCII nor
    /// typographic quotes can close a string and run code.
    #[test]
    fn toast_text_never_becomes_powershell_source() {
        let evil = "\u{2019}); Start-Process calc; (\u{2018}' ; calc";
        let c = toast_command("t\u{201B}itle", evil);
        let args: Vec<String> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args.last().map(String::as_str), Some(TOAST_SCRIPT));
        assert!(args.iter().all(|a| !a.contains("Start-Process")));
        let env: std::collections::HashMap<String, String> = c
            .get_envs()
            .filter_map(|(k, v)| {
                Some((
                    k.to_string_lossy().into_owned(),
                    v?.to_string_lossy().into_owned(),
                ))
            })
            .collect();
        assert_eq!(
            env.get("DEVPLANE_TOAST_BODY").map(String::as_str),
            Some(evil)
        );
        assert_eq!(
            env.get("DEVPLANE_TOAST_TITLE").map(String::as_str),
            Some("t\u{201B}itle")
        );
    }

    /// A body that looks like an option is still the body.
    #[test]
    fn notify_send_ends_its_options_before_the_text() {
        let c = notify_send_command("-t", "--help");
        let args: Vec<_> = c.get_args().map(|a| a.to_string_lossy()).collect();
        assert_eq!(args, ["--app-name=Devplane", "--", "-t", "--help"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn applescript_strings_are_escaped() {
        // A tool name is attacker-influenced: it must stay data in the script.
        let evil = r#"x" & (do shell script "touch /tmp/pwned") & ""#;
        let escaped = applescript_escape(evil);
        // Every quote must be escaped so none can close the string literal.
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

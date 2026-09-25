//! Deep links, and opening a worktree.
//!
//! `devplane://change|review|ask|run/<id>` and `devplane://inbox` open the
//! window on that object; an unknown link opens the inbox saying which links
//! exist. The bundle registers the scheme; Linux and Windows dev builds
//! register themselves, macOS needs an installed bundle.

use crate::core::ids::{AskId, ChangeId, RunId};
use tauri::AppHandle;
use tauri_plugin_deep_link::DeepLinkExt;

/// What a link names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Change(ChangeId),
    /// The same change, opened on its review.
    Review(ChangeId),
    Ask(AskId),
    Run(RunId),
    Inbox,
}

/// The kinds a link may name, for the refusal.
pub const KINDS: &str = "change, review, ask, run, inbox";

/// Reads a link. The scheme must be `devplane`; the host names the kind; the
/// path is the id, and an empty one is refused rather than opened on nothing.
pub fn parse(url: &str) -> Result<Target, String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("`{url}` has no scheme"))?;
    if !scheme.eq_ignore_ascii_case("devplane") {
        return Err(format!("`{url}` is not a devplane link"));
    }
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (kind, id) = match rest.split_once('/') {
        Some((k, i)) => (k, i.trim_matches('/')),
        None => (rest, ""),
    };
    match (kind.to_ascii_lowercase().as_str(), id) {
        ("inbox", _) => Ok(Target::Inbox),
        (_, "") => Err(format!("`{url}` names no id")),
        ("change", id) => Ok(Target::Change(ChangeId::new(id))),
        ("review", id) => Ok(Target::Review(ChangeId::new(id))),
        ("ask", id) => Ok(Target::Ask(AskId::new(id))),
        ("run", id) => Ok(Target::Run(RunId::new(id))),
        _ => Err(format!(
            "`{url}` names nothing this app opens; links are {KINDS}"
        )),
    }
}

/// The page query that lands on the target: `change=…`, `ask=…`, `run=…`,
/// and nothing for the inbox.
pub fn query_for(t: &Target) -> String {
    let esc = crate::core::text::url_escape;
    match t {
        Target::Change(id) => format!("change={}", esc(id.as_str())),
        Target::Review(id) => format!("review={}", esc(id.as_str())),
        Target::Ask(id) => format!("ask={}", esc(id.as_str())),
        Target::Run(id) => format!("run={}", esc(id.as_str())),
        Target::Inbox => String::new(),
    }
}

/// The query for a link as delivered: the target's, or the inbox with the
/// link the page could not follow, so it can say so.
pub fn query_for_url(url: &str) -> String {
    match parse(url) {
        Ok(t) => query_for(&t),
        Err(why) => {
            tracing::warn!("{why}");
            format!("link={}", crate::core::text::url_escape(url))
        }
    }
}

/// Where the app was asked to start, if a link started it: Linux and Windows
/// pass it on the command line, macOS through the plugin's own event.
pub fn startup_query(app: &AppHandle) -> String {
    match app.deep_link().get_current() {
        Ok(Some(urls)) if !urls.is_empty() => query_for_url(urls[0].as_str()),
        _ => String::new(),
    }
}

/// Links delivered while the app runs, and a second launch's arguments.
pub fn install(app: &AppHandle) {
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            super::window::show_main(&handle, &query_for_url(url.as_str()));
        }
    });
    // A dev build on Linux or Windows can register itself; macOS registers a
    // scheme only for an installed bundle, and says nothing here.
    #[cfg(any(target_os = "linux", windows))]
    if let Err(e) = app.deep_link().register_all() {
        tracing::warn!(error = %e, "could not register the devplane:// scheme");
    }
}

/// The second launch's arguments, handed over by the single-instance plugin:
/// a link among them opens here, and anything else shows the window.
pub fn from_argv(app: &AppHandle, argv: &[String]) {
    let query = argv
        .iter()
        .find(|a| a.starts_with("devplane://"))
        .map(|a| query_for_url(a))
        .unwrap_or_default();
    super::window::show_main(app, &query);
}

/// Opens a worktree in the editor or a terminal.
///
/// The editor is `$VISUAL`, then `$EDITOR`; with neither, the platform's file
/// manager opens the folder rather than guessing an editor.
pub fn open_in(
    app: &AppHandle,
    place: crate::host::OpenIn,
    path: &std::path::Path,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let shown = path.display().to_string();
    match place {
        crate::host::OpenIn::Editor => {
            let editor = std::env::var("VISUAL")
                .ok()
                .filter(|e| !e.trim().is_empty())
                .or_else(|| {
                    std::env::var("EDITOR")
                        .ok()
                        .filter(|e| !e.trim().is_empty())
                });
            match editor {
                Some(cmd) => spawn(&format!("{cmd} {}", shell_quote(&shown))),
                None => app
                    .opener()
                    .open_path(shown, None::<&str>)
                    .map_err(|e| e.to_string()),
            }
        }
        crate::host::OpenIn::Terminal => terminal(app, &shown),
    }
}

#[cfg(target_os = "macos")]
fn terminal(app: &AppHandle, path: &str) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path, Some("Terminal"))
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn terminal(_app: &AppHandle, path: &str) -> Result<(), String> {
    // The Debian alternative first, then the two common ones.
    for cmd in [
        format!(
            "x-terminal-emulator --working-directory={}",
            shell_quote(path)
        ),
        format!("gnome-terminal --working-directory={}", shell_quote(path)),
        format!("konsole --workdir {}", shell_quote(path)),
    ] {
        if spawn(&cmd).is_ok() {
            return Ok(());
        }
    }
    Err("no terminal found: x-terminal-emulator, gnome-terminal or konsole".into())
}

#[cfg(windows)]
fn terminal(_app: &AppHandle, path: &str) -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", "cmd", "/K", "cd", "/D", path])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn terminal(_app: &AppHandle, _path: &str) -> Result<(), String> {
    Err("no terminal on this platform".into())
}

/// Runs a command line through the shell, detached; the app waits for
/// nothing and reaps it on a thread of its own.
fn spawn(line: &str) -> Result<(), String> {
    #[cfg(unix)]
    let child = std::process::Command::new("sh")
        .arg("-c")
        .arg(line)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    #[cfg(windows)]
    let child = std::process::Command::new("cmd")
        .args(["/C", line])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match child {
        Ok(mut c) => {
            std::thread::Builder::new()
                .name("devplane-open".into())
                .spawn(move || {
                    let _ = c.wait();
                })
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_kinds_parse_and_land_on_their_object() {
        assert_eq!(
            parse("devplane://change/c-1").unwrap(),
            Target::Change(ChangeId::new("c-1"))
        );
        assert_eq!(
            parse("devplane://ask/a-2").unwrap(),
            Target::Ask(AskId::new("a-2"))
        );
        assert_eq!(
            parse("devplane://run/r-3/").unwrap(),
            Target::Run(RunId::new("r-3"))
        );
        assert_eq!(parse("devplane://inbox").unwrap(), Target::Inbox);
        assert_eq!(parse("DEVPLANE://Inbox/").unwrap(), Target::Inbox);

        assert_eq!(
            query_for(&Target::Change(ChangeId::new("c 1"))),
            "change=c%201"
        );
        assert_eq!(query_for(&Target::Ask(AskId::new("a-2"))), "ask=a-2");
        assert_eq!(
            parse("devplane://review/c-1").unwrap(),
            Target::Review(ChangeId::new("c-1"))
        );
        assert_eq!(
            query_for(&Target::Review(ChangeId::new("c-1"))),
            "review=c-1"
        );
        assert_eq!(query_for(&Target::Run(RunId::new("r-3"))), "run=r-3");
        assert_eq!(query_for(&Target::Inbox), "");
    }

    #[test]
    fn what_is_refused_says_why() {
        let unknown = parse("devplane://project/x").unwrap_err();
        assert!(
            unknown.contains("links are change, review, ask, run, inbox"),
            "{unknown}"
        );
        let scheme = parse("https://example.com/change/x").unwrap_err();
        assert!(scheme.contains("not a devplane link"), "{scheme}");
        let empty = parse("devplane://change/").unwrap_err();
        assert!(empty.contains("names no id"), "{empty}");
        assert!(parse("devplane://change").is_err());
        assert!(parse("nonsense").is_err());
        // The inbox carries the link, so the page can say what it could not
        // open.
        assert_eq!(
            query_for_url("devplane://project/x"),
            "link=devplane%3A%2F%2Fproject%2Fx"
        );
    }

    #[test]
    fn a_path_is_quoted_for_the_shell() {
        assert_eq!(shell_quote("/a b/c'd"), "'/a b/c'\\''d'");
    }
}

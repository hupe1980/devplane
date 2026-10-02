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
        // The change surface opens `<id>/review` on its Review tab.
        Target::Review(id) => format!("change={}/review", esc(id.as_str())),
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
    #[cfg(windows)]
    if let Err(e) = app.deep_link().register_all() {
        tracing::warn!(error = %e, "could not register the devplane:// scheme");
    }
    // On Linux the plugin writes a desktop file naming `$APPIMAGE` (never the
    // mount it runs from) or this binary. An AppImage has no installer, so
    // the first run is where links start to open it, and it says so once:
    // when the file did not exist before. Nothing is remembered.
    #[cfg(target_os = "linux")]
    {
        let desktop = handler_file(app);
        let existed = desktop.as_ref().is_some_and(|d| d.exists());
        match app.deep_link().register_all() {
            Err(e) => tracing::warn!(error = %e, "could not register the devplane:// scheme"),
            Ok(()) if !existed && std::env::var_os("APPIMAGE").is_some() => crate::notify::send(
                "Devplane",
                "devplane:// links now open this AppImage. Move or rename it and they stop \
                 until it runs again.",
            ),
            Ok(()) => {}
        }
    }
}

/// The desktop file the deep-link plugin writes on Linux: named after this
/// executable, in the user's applications folder.
#[cfg(target_os = "linux")]
fn handler_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;
    let bin = tauri::utils::platform::current_exe().ok()?;
    let name = format!("{}-handler.desktop", bin.file_name()?.to_string_lossy());
    Some(app.path().data_dir().ok()?.join("applications").join(name))
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
/// manager opens the folder rather than guessing an editor. Nothing here goes
/// through a shell: the path is always one argument of its own, so no
/// character in it can start a second command.
pub fn open_in(
    app: &AppHandle,
    place: crate::host::OpenIn,
    path: &std::path::Path,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
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
                Some(cmd) => {
                    #[allow(unused_mut)]
                    let mut launch = editor_launch(&cmd, path)?;
                    // Windows finds only `.exe` on its own; `code` is
                    // `code.cmd`, so the rest of `PATHEXT` is searched here.
                    #[cfg(windows)]
                    if let Some(found) = resolve_on_path(
                        &launch.program,
                        &std::env::var_os("PATH").unwrap_or_default(),
                        &std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into()),
                        |p| p.is_file(),
                    ) {
                        launch.program = found.into_os_string();
                    }
                    spawn(&launch)
                        .map_err(|e| format!("could not start the editor `{}`: {e}", cmd.trim()))
                }
                None => app
                    .opener()
                    .open_path(path.display().to_string(), None::<&str>)
                    .map_err(|e| e.to_string()),
            }
        }
        crate::host::OpenIn::Terminal => terminal(app, path),
    }
}

/// A program and its arguments, started directly: no shell reads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: std::ffi::OsString,
    pub args: Vec<std::ffi::OsString>,
    /// Where it starts.
    pub cwd: Option<std::path::PathBuf>,
    /// Windows only: a console window of its own.
    pub new_console: bool,
}

impl Launch {
    fn new(program: impl Into<std::ffi::OsString>) -> Self {
        Launch {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            new_console: false,
        }
    }

    fn arg(mut self, a: impl Into<std::ffi::OsString>) -> Self {
        self.args.push(a.into());
        self
    }

    fn command(&self) -> std::process::Command {
        let mut c = std::process::Command::new(&self.program);
        c.args(&self.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if let Some(dir) = &self.cwd {
            c.current_dir(dir);
        }
        #[cfg(windows)]
        if self.new_console {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
            c.creation_flags(CREATE_NEW_CONSOLE);
        }
        c
    }
}

/// The editor command line (`code --wait`, `"C:\Program Files\x\ed.exe" -n`)
/// split into words, with the path appended as one more argument.
pub fn editor_launch(editor: &str, path: &std::path::Path) -> Result<Launch, String> {
    let mut words = split_words(editor)?.into_iter();
    let program = words
        .next()
        .ok_or_else(|| "the editor variable names no program".to_string())?;
    let mut launch = Launch::new(program);
    launch.args.extend(words.map(Into::into));
    launch.args.push(path.as_os_str().to_owned());
    Ok(launch)
}

/// Splits a command line into words: blanks separate, and single or double
/// quotes group. A backslash is an ordinary character, so a Windows path in
/// the variable survives as written.
pub fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for ch in line.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => word.push(ch),
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                in_word = true;
            }
            None if ch.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(ch);
                in_word = true;
            }
        }
    }
    if let Some(q) = quote {
        return Err(format!("the editor variable `{line}` leaves a {q} open"));
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

/// A bare program name found on `PATH` under one of `PATHEXT`'s extensions,
/// the way Windows' own shell would. A name with a directory or an extension
/// is left as written.
pub fn resolve_on_path(
    program: &std::ffi::OsStr,
    path_var: &std::ffi::OsStr,
    pathext: &str,
    exists: impl Fn(&std::path::Path) -> bool,
) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(program);
    if p.components().count() != 1 || p.extension().is_some() {
        return None;
    }
    let exts: Vec<&str> = pathext.split(';').filter(|e| !e.is_empty()).collect();
    std::env::split_paths(path_var).find_map(|dir| {
        exts.iter().find_map(|ext| {
            let mut name = program.to_owned();
            name.push(ext);
            let candidate = dir.join(name);
            exists(&candidate).then_some(candidate)
        })
    })
}

/// The terminals tried on Linux, in order: the Debian alternative first, then
/// the two common ones. Each starts in the folder and is told it as well.
pub fn linux_terminals(path: &std::path::Path) -> Vec<Launch> {
    let flag = |name: &str| {
        let mut f = std::ffi::OsString::from(name);
        f.push(path.as_os_str());
        f
    };
    let at = |l: Launch| Launch {
        cwd: Some(path.to_path_buf()),
        ..l
    };
    vec![
        at(Launch::new("x-terminal-emulator").arg(flag("--working-directory="))),
        at(Launch::new("gnome-terminal").arg(flag("--working-directory="))),
        at(Launch::new("konsole")
            .arg("--workdir")
            .arg(path.as_os_str())),
    ]
}

/// The terminal on Windows: a console of its own, started in the folder. The
/// folder is the working directory, never part of a command line `cmd` reads.
pub fn windows_terminal(path: &std::path::Path) -> Launch {
    Launch {
        cwd: Some(path.to_path_buf()),
        new_console: true,
        ..Launch::new("cmd.exe").arg("/K")
    }
}

/// Starts the first candidate that spawns. One that cannot start (not
/// installed, not executable) is passed over for the next; the refusal names
/// every one tried and why.
pub fn first_that_spawns(
    candidates: &[Launch],
    mut spawn: impl FnMut(&Launch) -> std::io::Result<()>,
) -> Result<(), String> {
    let mut tried = Vec::new();
    for c in candidates {
        match spawn(c) {
            Ok(()) => return Ok(()),
            Err(e) => tried.push(format!("{} ({e})", c.program.to_string_lossy())),
        }
    }
    Err(format!(
        "no terminal could be started; tried {}",
        tried.join(", ")
    ))
}

#[cfg(target_os = "macos")]
fn terminal(app: &AppHandle, path: &std::path::Path) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path.display().to_string(), Some("Terminal"))
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn terminal(_app: &AppHandle, path: &std::path::Path) -> Result<(), String> {
    first_that_spawns(&linux_terminals(path), spawn)
}

#[cfg(windows)]
fn terminal(_app: &AppHandle, path: &std::path::Path) -> Result<(), String> {
    spawn(&windows_terminal(path)).map_err(|e| e.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn terminal(_app: &AppHandle, _path: &std::path::Path) -> Result<(), String> {
    Err("no terminal on this platform".into())
}

/// Starts a program, detached; the app waits for nothing and reaps it on a
/// thread of its own. An error means the process never started.
fn spawn(launch: &Launch) -> std::io::Result<()> {
    let mut child = launch.command().spawn()?;
    std::thread::Builder::new()
        .name("devplane-open".into())
        .spawn(move || {
            let _ = child.wait();
        })?;
    Ok(())
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
            "change=c-1/review"
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

    /// No candidate is a shell, and nothing in a launch reads like one.
    fn no_shell(l: &Launch) {
        let prog = l.program.to_string_lossy().to_ascii_lowercase();
        for shell in ["sh", "bash", "zsh", "cmd", "cmd.exe", "powershell"] {
            if prog == shell || prog.ends_with(&format!("/{shell}")) {
                assert!(
                    !l.args.iter().any(|a| a == "-c" || a == "/C" || a == "/c"),
                    "{l:?} runs a command line through {shell}"
                );
            }
        }
    }

    #[test]
    fn the_editor_gets_the_path_as_one_argument_and_no_shell() {
        for path in [
            r"C:\a & calc.exe\b.rs",
            "/tmp/a;rm -rf x",
            "/tmp/a|b $(x) `y`",
        ] {
            let l = editor_launch("code --wait", std::path::Path::new(path)).unwrap();
            assert_eq!(l.program, "code");
            assert_eq!(
                l.args,
                vec![std::ffi::OsString::from("--wait"), path.into()]
            );
            no_shell(&l);
        }
        let l = editor_launch(
            r#""C:\Program Files\Ed\ed.exe" -n"#,
            std::path::Path::new("x.rs"),
        )
        .unwrap();
        assert_eq!(l.program, r"C:\Program Files\Ed\ed.exe");
        assert_eq!(l.args, vec![std::ffi::OsString::from("-n"), "x.rs".into()]);
        assert!(editor_launch("   ", std::path::Path::new("x")).is_err());
        assert!(editor_launch("'vim", std::path::Path::new("x")).is_err());
    }

    #[test]
    fn a_bare_editor_is_found_under_pathext() {
        let dir = std::path::Path::new("bin");
        let path_var = std::env::join_paths([dir]).unwrap();
        let found = resolve_on_path(std::ffi::OsStr::new("code"), &path_var, ".EXE;.CMD", |p| {
            p == dir.join("code.CMD")
        });
        assert_eq!(found, Some(dir.join("code.CMD")));
        // Named with an extension or a directory: left as written.
        assert_eq!(
            resolve_on_path(std::ffi::OsStr::new("code.exe"), &path_var, ".CMD", |_| {
                true
            }),
            None
        );
    }

    #[test]
    fn terminals_take_the_folder_as_an_argument_never_a_command_line() {
        let path = std::path::Path::new("/tmp/a;rm -rf x");
        let all = linux_terminals(path);
        assert_eq!(
            all.iter()
                .map(|l| l.program.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["x-terminal-emulator", "gnome-terminal", "konsole"]
        );
        for l in &all {
            no_shell(l);
            assert_eq!(l.cwd.as_deref(), Some(path));
        }
        assert_eq!(
            all[0].args,
            vec![std::ffi::OsString::from(
                "--working-directory=/tmp/a;rm -rf x"
            )]
        );
        assert_eq!(
            all[2].args,
            vec![std::ffi::OsString::from("--workdir"), path.into()]
        );

        let win = windows_terminal(std::path::Path::new(r"C:\a & calc.exe"));
        no_shell(&win);
        assert_eq!(win.args, vec![std::ffi::OsString::from("/K")]);
        assert_eq!(
            win.cwd.as_deref(),
            Some(std::path::Path::new(r"C:\a & calc.exe"))
        );
        assert!(win.new_console);
    }

    #[test]
    fn a_missing_terminal_is_passed_over_for_the_next() {
        let all = linux_terminals(std::path::Path::new("/w"));
        let mut asked = Vec::new();
        let got = first_that_spawns(&all, |l| {
            asked.push(l.program.clone());
            match l.program.to_str() {
                Some("konsole") => Ok(()),
                _ => Err(std::io::ErrorKind::NotFound.into()),
            }
        });
        assert_eq!(got, Ok(()));
        assert_eq!(asked.len(), 3);

        let none =
            first_that_spawns(&all, |_| Err(std::io::ErrorKind::NotFound.into())).unwrap_err();
        for name in ["x-terminal-emulator", "gnome-terminal", "konsole"] {
            assert!(none.contains(name), "{none}");
        }

        // The first that starts is the only one started.
        let mut count = 0;
        assert_eq!(
            first_that_spawns(&all, |_| {
                count += 1;
                Ok(())
            }),
            Ok(())
        );
        assert_eq!(count, 1);
    }
}

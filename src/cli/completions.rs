//! Shell completion, generated from the command tree rather than written twice.
//!
//! # Why it is generated
//!
//! Thirty-five commands, and their ids are this product's own vocabulary: an
//! `AskId`, a `RunId`, a project name. Hand-written scripts for three shells go
//! stale the first time a subcommand is added, and the failure is silent —
//! nothing tells you your completions are a release behind.
//!
//! So the script comes from the same [`clap::Command`] the help screen renders.
//! **A command that exists completes; a hidden one is neither documented nor
//! offered**, and a test reads both from that one tree.
//!
//! # Two guarantees, and they are different
//!
//! **Static completion needs no daemon.** `devplane completions zsh` is a pure
//! function of the binary: it writes a script and talks to nothing. Somebody
//! setting up a shell should not have to have started anything.
//!
//! **Live completion needs one and is silent without it.** Completing an
//! `AskId` means asking what is waiting, and there is no honest answer when
//! nothing is running. It **connects or returns nothing** — never
//! `connect_or_start`, because pressing Tab must not launch a daemon — and it
//! gives up after [`LIVE_TIMEOUT`], because a shell that hangs on Tab is worse
//! than one that completes nothing.

use anyhow::Result;
use clap::CommandFactory;
use std::io::Write;
use std::time::Duration;

/// How long a Tab press may wait on the daemon.
///
/// **A budget for a keystroke, not for a request.** Past this a shell feels
/// broken, and the honest failure is an empty completion rather than a pause.
pub const LIVE_TIMEOUT: Duration = Duration::from_millis(150);

/// The most values one completion offers.
///
/// A person picking from a list is not reading four hundred of them, and a
/// shell rendering them is worse. Ordered as the inbox orders — what is waiting
/// first, then by age — so the bound keeps the useful end.
pub const MAX_VALUES: usize = 50;

/// The shells with a generator here.
pub const SHELLS: &[&str] = &["bash", "zsh", "fish"];

/// Writes the completion script for `shell`.
///
/// An unsupported shell names the ones that are supported and **writes
/// nothing**: a half-written script sourced by a shell profile is a broken
/// prompt on every new terminal.
pub fn cmd_completions(shell: &str) -> Result<()> {
    let generator = match shell.to_ascii_lowercase().as_str() {
        "bash" => clap_complete::Shell::Bash,
        "zsh" => clap_complete::Shell::Zsh,
        "fish" => clap_complete::Shell::Fish,
        other => {
            anyhow::bail!(
                "no completions for `{other}`. This command writes: {}",
                SHELLS.join(", ")
            );
        }
    };
    let mut cmd = offered();
    let mut out = std::io::stdout().lock();
    clap_complete::generate(generator, &mut cmd, "devplane", &mut out);
    // **The live half, appended.** `clap_complete` generates a static tree; the
    // ids this product owns — a waiting ask, a session, a project — are only
    // knowable by asking the daemon, so the script calls back into the binary.
    out.write_all(live_snippet(generator).as_bytes())?;
    out.flush()?;
    Ok(())
}

/// The shell-specific lines that complete the ids.
///
/// **zsh and fish get them; bash does not**, and that is written down rather
/// than left to be discovered. Both of the first two take a description beside
/// each value, which is what makes completing an opaque `AskId` useful — the id
/// alone tells you nothing, and the question beside it tells you everything.
/// Bash completes words with no descriptions, so an id list there is a column
/// of ULIDs to choose between, which is not an improvement on typing one.
fn live_snippet(shell: clap_complete::Shell) -> String {
    let arg = super::COMPLETE_ARG;
    match shell {
        clap_complete::Shell::Zsh => format!(
            r#"
# --- devplane live values -------------------------------------------------
# Asks the running daemon. Silent when there is none: pressing Tab must not
# start a daemon, and an empty completion is the honest answer.
_devplane_live() {{
  local -a out
  out=("${{(@f)$(devplane {arg} "$1" 2>/dev/null)}}")
  [[ -n "$out" ]] || return 1
  _describe -t devplane "$2" out
}}
compdef '_devplane_live asks "waiting question"' devplane-answer
compdef '_devplane_live runs "session"' devplane-show devplane-say devplane-snooze
compdef '_devplane_live projects "project"' devplane-trust
"#
        ),
        clap_complete::Shell::Fish => format!(
            r#"
# --- devplane live values -------------------------------------------------
# Silent without a daemon: pressing Tab must not start one.
function __devplane_live
    devplane {arg} $argv[1] 2>/dev/null
end
complete -c devplane -n "__fish_seen_subcommand_from answer" -f   -a "(__devplane_live asks)" -d "waiting question"
complete -c devplane -n "__fish_seen_subcommand_from show say snooze" -f   -a "(__devplane_live runs)" -d "session"
complete -c devplane -n "__fish_seen_subcommand_from trust" -f   -a "(__devplane_live projects)" -d "project"
"#
        ),
        // **Bash, deliberately not.** See [`live_snippet`].
        _ => String::new(),
    }
}

/// The command tree **as a person should see it**.
///
/// **`hide = true` is honoured by the help screen and not by `clap_complete`.**
/// A hidden command came out of the generator with its description attached, so
/// `devplane mcp` — hidden precisely because *"an agent runs this, not a
/// person, and a listing a person reads is shorter and truer without it"* —
/// was offered in every completion list. A completion list is a listing a
/// person reads.
///
/// So the tree is filtered before it is generated from, rather than the script
/// being filtered after: a text filter over generated output is a second thing
/// to keep true, and it breaks silently the first time a generator changes its
/// quoting.
fn offered() -> clap::Command {
    let full = super::Cli::command();
    let visible: Vec<clap::Command> = full
        .get_subcommands()
        .filter(|c| !c.is_hide_set())
        .cloned()
        .collect();
    // Everything about the root except its subcommands, then only the visible
    // ones. Rebuilt rather than mutated because `clap::Command` has no public
    // way to remove one.
    let mut root = clap::Command::new("devplane")
        .about(full.get_about().map(|a| a.to_string()).unwrap_or_default())
        .version(env!("CARGO_PKG_VERSION"));
    for arg in full.get_arguments() {
        root = root.arg(arg.clone());
    }
    root.subcommands(visible)
}

/// What a live completion can offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Values {
    /// Questions and permissions waiting for an answer.
    Asks,
    /// Sessions, so `show`, `say` and `snooze` complete.
    Runs,
    /// Registered projects, for `--to` and `--project`.
    Projects,
}

impl Values {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "asks" => Some(Values::Asks),
            "runs" => Some(Values::Runs),
            "projects" => Some(Values::Projects),
            _ => None,
        }
    }

    fn route(self) -> &'static str {
        match self {
            Values::Asks => "/api/asks",
            Values::Runs => "/api/board",
            Values::Projects => "/api/projects",
        }
    }
}

/// One value per line, with a label after a tab where there is one.
///
/// The format zsh and fish both read: `value<TAB>description`. Bash ignores the
/// second half, which is why `--help` says bash completes the ids without the
/// sentence beside them.
pub async fn cmd_complete(what: &str) -> Result<()> {
    let Some(kind) = Values::parse(what) else {
        // **Silence, not an error.** This runs on a keystroke; a message here
        // would be printed into somebody's command line.
        return Ok(());
    };

    let Some(body) = ask(kind).await else {
        return Ok(());
    };

    let mut out = std::io::stdout().lock();
    for (value, label) in values(kind, &body).into_iter().take(MAX_VALUES) {
        // **Escaped, because these are somebody else's strings.** A project
        // named `my project` and a question carrying a quote have to complete
        // into a command line that still runs.
        match label {
            Some(l) => writeln!(out, "{}\t{}", escape(&value), one_line(&l))?,
            None => writeln!(out, "{}", escape(&value))?,
        }
    }
    out.flush()?;
    Ok(())
}

/// Asks the daemon, or gives up.
///
/// **Connects, never starts.** `connect_or_start` would make a Tab press launch
/// a daemon — a keystroke with a side effect, and one that takes seconds.
async fn ask(kind: Values) -> Option<serde_json::Value> {
    let c = crate::client::Client::connect().ok()?;
    tokio::time::timeout(LIVE_TIMEOUT, c.get::<serde_json::Value>(kind.route()))
        .await
        .ok()?
        .ok()
}

/// Pulls the values out of what the route served.
///
/// Ordered as the source orders. The inbox is already ranked by what is waiting
/// and then by age, and re-sorting here would be a second opinion about urgency
/// in a completion list.
fn values(kind: Values, body: &serde_json::Value) -> Vec<(String, Option<String>)> {
    let arr = |v: &serde_json::Value| v.as_array().cloned().unwrap_or_default();
    match kind {
        Values::Asks => arr(body)
            .iter()
            .filter_map(|a| {
                let id = a.get("id")?.as_str()?.to_string();
                let label = a
                    .get("question")
                    .and_then(|q| q.as_str())
                    .or_else(|| a.get("tool").and_then(|t| t.as_str()))
                    .map(str::to_string);
                Some((id, label))
            })
            .collect(),
        Values::Runs => arr(body.get("runs").unwrap_or(&serde_json::Value::Null))
            .iter()
            .filter_map(|r| {
                let id = r.get("id")?.as_str()?.to_string();
                let label = r
                    .get("project_name")
                    .and_then(|p| p.as_str())
                    .map(str::to_string);
                Some((id, label))
            })
            .collect(),
        Values::Projects => arr(body)
            .iter()
            .filter_map(|p| {
                let name = p.get("name")?.as_str()?.to_string();
                Some((name, None))
            })
            .collect(),
    }
}

/// Makes a value safe to put on a command line.
///
/// Anything outside a conservative unreserved set means the whole value is
/// single-quoted, with embedded single quotes closed and re-opened the way
/// every POSIX shell reads them. Over-quoting costs nothing; under-quoting
/// makes `devplane ls --project my project` two arguments.
fn escape(s: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "-_./:@+=".contains(c);
    if !s.is_empty() && s.chars().all(safe) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// A label is one line, because a completion list is one line per value.
fn one_line(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    crate::core::text::clip(flat.trim(), 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every command the help screen shows is a command that completes, and
    /// every hidden one is neither.**
    ///
    /// Read from the one `clap::Command` both are rendered from, so this cannot
    /// pass with a list somebody forgot to update — which is the whole reason
    /// the script is generated rather than written.
    #[test]
    fn the_completion_tree_is_the_help_tree() {
        let cmd = crate::cli::Cli::command();
        let visible: Vec<&str> = cmd
            .get_subcommands()
            .filter(|c| !c.is_hide_set())
            .map(|c| c.get_name())
            .collect();
        assert!(
            visible.len() > 20,
            "only {} commands are visible; this guard has stopped reading the tree",
            visible.len()
        );

        let mut buf: Vec<u8> = Vec::new();
        let mut c = offered();
        clap_complete::generate(clap_complete::Shell::Zsh, &mut c, "devplane", &mut buf);
        let script = String::from_utf8(buf).expect("a completion script is utf-8");

        for name in &visible {
            assert!(
                script.contains(name),
                "`devplane {name}` is on the help screen and not in the completion script"
            );
        }
        // **Nothing hidden is offered, and the helper is not in the tree at
        // all.** `hide = true` keeps a command off the help screen and *not*
        // out of `clap_complete`'s output — the generated zsh script carried
        // the live-values helper as an offered command with its description.
        // `main` answers it before clap parses, so clap cannot emit it.
        for hidden in cmd.get_subcommands().filter(|c| c.is_hide_set()) {
            let n = hidden.get_name();
            assert!(
                !script.contains(&format!("'{n}:")),
                "`{n}` is hidden from the help screen and offered by the completions"
            );
        }
        assert!(
            !script.contains(crate::cli::COMPLETE_ARG) && !script.contains("'complete:"),
            "the live-values helper is back in the command tree, so it is back in \
             every generated completion script"
        );
    }

    /// An unsupported shell writes nothing and names the ones that work.
    #[test]
    fn an_unknown_shell_is_refused_by_name() {
        let e = cmd_completions("powershell").expect_err("an unsupported shell is an error");
        let says = e.to_string();
        for s in SHELLS {
            assert!(says.contains(s), "the refusal does not name `{s}`: {says}");
        }
    }

    /// Every supported shell actually generates.
    #[test]
    fn every_named_shell_writes_a_script() {
        for s in SHELLS {
            assert!(
                cmd_completions(s).is_ok(),
                "`{s}` is offered and does not generate"
            );
        }
    }

    /// **A value with a space, a quote or a newline completes into a command
    /// line that still runs.**
    #[test]
    fn values_are_escaped_for_a_shell() {
        assert_eq!(escape("payments-api"), "payments-api");
        assert_eq!(escape("my project"), "'my project'");
        assert_eq!(escape("it's here"), r"'it'\''s here'");
        assert_eq!(escape(""), "''");
        // A shell metacharacter must never reach the line bare.
        for bad in ["a;rm -rf /", "a&&b", "a|b", "$(id)", "`id`", "a\nb"] {
            let out = escape(bad);
            assert!(
                out.starts_with('\'') && out.ends_with('\''),
                "{bad:?} was not quoted: {out}"
            );
        }
    }

    /// **zsh and fish complete the ids; bash completes the commands only**, and
    /// that is a decision written down rather than left to be discovered.
    ///
    /// Both of the first two render a description beside each value, which is
    /// the whole point of completing an opaque id: the id tells you nothing and
    /// the question beside it tells you everything. Bash has no descriptions,
    /// so the same list there is a column of ULIDs.
    #[test]
    fn the_live_half_is_offered_where_a_description_can_be_shown() {
        for (sh, wants) in [
            (clap_complete::Shell::Zsh, true),
            (clap_complete::Shell::Fish, true),
            (clap_complete::Shell::Bash, false),
        ] {
            let snippet = live_snippet(sh);
            assert_eq!(
                snippet.contains(crate::cli::COMPLETE_ARG),
                wants,
                "{sh:?} live completion is {}",
                if wants {
                    "missing"
                } else {
                    "offered and cannot show a label"
                }
            );
            if wants {
                for kind in ["asks", "runs", "projects"] {
                    assert!(snippet.contains(kind), "{sh:?} does not complete {kind}");
                }
                assert!(
                    snippet.contains("2>/dev/null"),
                    "{sh:?} lets the daemon's absence print into the command line"
                );
            }
        }
    }

    /// **Pressing Tab does not start a daemon, and says nothing when there is
    /// none.**
    ///
    /// The whole safety argument for the live half. `connect_or_start` would
    /// make a keystroke launch a process that takes seconds — and would do it
    /// from a shell that is waiting to draw a prompt.
    ///
    /// Asserted two ways, because one of them can rot: the source may not name
    /// `connect_or_start` on this path, and running it with no daemon must
    /// produce no output and no error.
    #[tokio::test]
    async fn a_keystroke_never_starts_a_daemon() {
        // **The implementation half, and a call rather than the word.** Two
        // ways to fail at reading a file for a forbidden call, both hit here:
        // the prose above explains *why* this path avoids that constructor, and
        // the assertion below names it in a literal. A guard that matches its
        // own message is one that can only be satisfied by deleting itself.
        let whole = include_str!("completions.rs");
        let code = whole.split("#[cfg(test)]").next().unwrap_or(whole);
        let forbidden = format!("connect_or{}(", "_start");
        assert!(
            !code.contains(&forbidden),
            "the completion path can start a daemon from a Tab press"
        );
        assert!(
            code.contains("Client::connect()"),
            "the completion path no longer connects at all, so this guards nothing"
        );

        // With `DEVPLANE_HOME` pointed at an empty directory there is no daemon
        // info to read, so `connect` fails and this must be silent.
        let empty =
            std::env::temp_dir().join(format!("dp-nodaemon-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&empty).unwrap();
        // SAFETY: single-threaded within this test's scope, and the value is
        // restored before it returns.
        let before = std::env::var_os("DEVPLANE_HOME");
        unsafe { std::env::set_var("DEVPLANE_HOME", &empty) };
        let out = cmd_complete("projects").await;
        match before {
            Some(v) => unsafe { std::env::set_var("DEVPLANE_HOME", v) },
            None => unsafe { std::env::remove_var("DEVPLANE_HOME") },
        }
        let _ = std::fs::remove_dir_all(&empty);
        assert!(
            out.is_ok(),
            "completing with no daemon was an error rather than silence"
        );
    }

    /// **The budget is for a keystroke, not for a request.**
    ///
    /// Past 150 ms a shell feels broken, and the honest failure is an empty
    /// completion rather than a pause. Held as a bound on the constant rather
    /// than by timing a call, because a timing assertion on a loaded machine
    /// fails for reasons that have nothing to do with the code — which is the
    /// rule the rest of this repository's measurements already follow.
    #[test]
    fn the_live_budget_is_a_keystroke() {
        assert!(
            LIVE_TIMEOUT <= Duration::from_millis(200),
            "{LIVE_TIMEOUT:?} is long enough for a shell to feel broken"
        );
        assert!(
            LIVE_TIMEOUT >= Duration::from_millis(50),
            "{LIVE_TIMEOUT:?} is short enough to lose a healthy daemon's answer"
        );
        // And the timeout is actually applied, rather than being a constant
        // somebody wrote down beside a call that ignores it.
        let src = include_str!("completions.rs");
        assert!(
            src.contains("tokio::time::timeout(LIVE_TIMEOUT"),
            "LIVE_TIMEOUT is declared and not applied"
        );
    }

    /// A label is one line, whatever the agent wrote.
    #[test]
    fn a_label_cannot_break_the_list() {
        let l = one_line("two\nlines\tand\ta tab");
        assert!(!l.contains('\n') && !l.contains('\t'), "{l:?}");
    }

    /// An unknown value set is **silence**, because this runs on a keystroke.
    #[tokio::test]
    async fn an_unknown_value_set_says_nothing() {
        assert!(cmd_complete("nonsense").await.is_ok());
        assert_eq!(Values::parse("nonsense"), None);
        for k in ["asks", "runs", "projects"] {
            assert!(Values::parse(k).is_some(), "`{k}` is wired in the shell");
        }
    }

    /// The values come out of the shapes the routes actually serve.
    #[test]
    fn values_are_read_from_the_served_shapes() {
        let asks = serde_json::json!([{"id": "a1", "question": "Keep it?"}, {"id": "a2"}]);
        let v = values(Values::Asks, &asks);
        assert_eq!(v[0], ("a1".into(), Some("Keep it?".into())));
        assert_eq!(v[1], ("a2".into(), None));

        let board = serde_json::json!({"runs": [{"id": "r1", "project_name": "saas"}]});
        assert_eq!(
            values(Values::Runs, &board)[0],
            ("r1".into(), Some("saas".into()))
        );

        let projects = serde_json::json!([{"name": "payments-api"}]);
        assert_eq!(
            values(Values::Projects, &projects)[0],
            ("payments-api".into(), None)
        );
    }
}

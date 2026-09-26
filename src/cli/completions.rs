//! Shell completion, generated from the same [`clap::Command`] the help screen
//! renders, so a visible command completes and a hidden one is never offered.
//!
//! The static script needs no host. Live values (ask, run and change ids) are
//! read through [`crate::local::Reader`], which starts nothing, within
//! [`LIVE_TIMEOUT`]; an empty read completes nothing. zsh positionals are
//! rewritten to call a `_devplane_live` helper defined above clap's dispatch
//! line.

use anyhow::{Context, Result};
use clap::CommandFactory;
use std::io::Write;
use std::time::Duration;

/// How long a Tab press may wait; past it the completion is empty.
pub const LIVE_TIMEOUT: Duration = Duration::from_millis(150);

/// The most values one completion offers. Sources are ordered most-urgent
/// first, so the bound keeps the useful end.
pub const MAX_VALUES: usize = 50;

/// The shells with a generator here.
pub const SHELLS: &[&str] = &["bash", "zsh", "fish"];

/// Writes the completion script for `shell`. An unsupported shell writes
/// nothing and names the supported ones.
pub fn cmd_completions(shell: &str) -> Result<()> {
    let text = script(shell)?;
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes())?;
    out.flush()?;
    Ok(())
}

/// The whole completion script for `shell`, live half included.
///
/// Only zsh and fish get live values: they show a description beside each id,
/// and without one an opaque id list is no help, so bash gets none.
fn script(shell: &str) -> Result<String> {
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
    let mut buf: Vec<u8> = Vec::new();
    clap_complete::generate(generator, &mut cmd, "devplane", &mut buf);
    let generated = String::from_utf8(buf).context("a completion script is utf-8")?;
    Ok(match generator {
        clap_complete::Shell::Zsh => zsh_with_live_values(&generated),
        clap_complete::Shell::Fish => format!("{generated}{}", fish_live_values()),
        // Bash: no live values; see [`script`].
        _ => generated,
    })
}

/// The positionals that complete from live values: the value name clap
/// prints in the script, and the value set behind it.
const LIVE_POSITIONALS: &[(&str, Values)] = &[
    ("ask", Values::Asks),
    ("run", Values::Runs),
    ("change", Values::Changes),
];

/// Rewrites clap's zsh script so the live positionals dispatch to the helper.
///
/// The helper goes above clap's `if [ "$funcstack[1]" = "_devplane" ]` line,
/// which zsh runs while loading the file, so it exists on the first Tab. Each
/// `':run…:_default'` positional becomes `':run…:_devplane_live runs'`.
fn zsh_with_live_values(generated: &str) -> String {
    let arg = super::COMPLETE_ARG;
    let helper = format!(
        r#"# --- devplane live values -------------------------------------------------
# Reads what is waiting. Silent when nothing can be read: pressing Tab must
# not start anything, and an empty completion is the honest answer.
_devplane_live() {{
  local -a out
  out=("${{(@f)$(devplane {arg} "$1" 2>/dev/null)}}")
  [[ -n "$out" ]] || return 1
  out=("${{(@)out//$'	'/:}}")
  _describe -t devplane "$1" out
}}

"#
    );
    let mut out = String::with_capacity(generated.len() + helper.len());
    for line in generated.lines() {
        if line.starts_with(r#"if [ "$funcstack[1]" = "_devplane" ]"#) {
            out.push_str(&helper);
        }
        out.push_str(&live_positional(line).unwrap_or_else(|| line.to_string()));
        out.push('\n');
    }
    out
}

/// A clap positional line (`':<name>[ -- <help>]:_default' \`) rewritten to
/// dispatch live, or `None` for any other line. `<name>` is the field name in
/// the command tree, so a rename there fails a test here.
fn live_positional(line: &str) -> Option<String> {
    let rest = line.strip_prefix("':")?;
    let action = ":_default' \\";
    let head = line.strip_suffix(action)?;
    let name = rest.split([':', ' ']).next().unwrap_or_default();
    let (_, values) = LIVE_POSITIONALS.iter().find(|(n, _)| *n == name)?;
    Some(format!("{head}:_devplane_live {}' \\", values.name()))
}

/// The fish half. Fish reads a `value<TAB>description` list directly.
fn fish_live_values() -> String {
    let arg = super::COMPLETE_ARG;
    format!(
        r#"
# --- devplane live values -------------------------------------------------
# Silent when nothing can be read: pressing Tab must not start anything.
function __devplane_live
    devplane {arg} $argv[1] 2>/dev/null
end
complete -c devplane -n "__fish_seen_subcommand_from answer" -f   -a "(__devplane_live asks)" -d "waiting question"
complete -c devplane -n "__fish_seen_subcommand_from show watch snooze attach" -f   -a "(__devplane_live runs)" -d "session"
complete -c devplane -n "__fish_seen_subcommand_from change" -f   -a "(__devplane_live changes)" -d "change"
"#
    )
}

/// The command tree without hidden subcommands.
///
/// `clap_complete` ignores `hide = true`, so the tree is filtered before
/// generating rather than the script's text after.
fn offered() -> clap::Command {
    let full = super::Cli::command();
    let visible: Vec<clap::Command> = full
        .get_subcommands()
        .filter(|c| !c.is_hide_set())
        .cloned()
        .collect();
    // Rebuilt rather than mutated: `clap::Command` cannot remove a subcommand.
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
    /// Sessions, so `show`, `watch` and `attach` complete.
    Runs,
    /// Changes, for every `devplane change <verb> <change>`.
    Changes,
    /// Registered projects, for `--project`.
    Projects,
}

impl Values {
    /// Every value set, so the shell and binary sides share one list.
    pub const ALL: &'static [Values] = &[
        Values::Asks,
        Values::Runs,
        Values::Changes,
        Values::Projects,
    ];

    /// The word the shell passes back. One spelling, read by [`Self::parse`].
    pub fn name(self) -> &'static str {
        match self {
            Values::Asks => "asks",
            Values::Runs => "runs",
            Values::Changes => "changes",
            Values::Projects => "projects",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|v| v.name() == s)
    }

    fn route(self) -> &'static str {
        match self {
            Values::Asks => "/api/asks",
            Values::Runs => "/api/board",
            Values::Changes => "/api/changes",
            Values::Projects => "/api/projects",
        }
    }
}

/// One value per line, as `value<TAB>description` where there is a label —
/// the format zsh and fish read.
pub async fn cmd_complete(what: &str) -> Result<()> {
    let Some(kind) = Values::parse(what) else {
        // Silence, not an error: output here lands in the command line.
        return Ok(());
    };

    let Some(body) = ask(kind).await else {
        return Ok(());
    };

    let mut out = std::io::stdout().lock();
    for (value, label) in values(kind, &body).into_iter().take(MAX_VALUES) {
        // Escaped: project names and questions may carry spaces or quotes.
        match label {
            Some(l) => writeln!(out, "{}\t{}", escape(&value), one_line(&l))?,
            None => writeln!(out, "{}", escape(&value))?,
        }
    }
    out.flush()?;
    Ok(())
}

/// Reads the values, or gives up.
///
/// [`crate::local::Reader`] asks a running host or else the store, and never
/// starts anything. Opening and reading share the one budget.
async fn ask(kind: Values) -> Option<serde_json::Value> {
    tokio::time::timeout(LIVE_TIMEOUT, async {
        crate::local::Reader::open()
            .await
            .ok()?
            .get(kind.route())
            .await
            .ok()
    })
    .await
    .ok()
    .flatten()
}

/// Pulls the values out of what the route served, in the source's order.
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
        Values::Changes => arr(body)
            .iter()
            .filter_map(|w| {
                let id = w.get("id")?.as_str()?.to_string();
                let label = w.get("title").and_then(|t| t.as_str()).map(str::to_string);
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
/// Anything outside a conservative unreserved set is single-quoted, POSIX
/// style, with embedded quotes closed and re-opened.
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

    /// Every command the help screen shows completes; no hidden one does.
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
        // Nothing hidden is offered, and the live-values helper is not in the
        // tree at all (`main` answers it before clap parses).
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

    /// A value with a space, a quote or a newline completes into a command
    /// line that still runs.
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

    /// zsh and fish complete the ids; bash, which shows no descriptions,
    /// completes the commands only.
    #[test]
    fn the_live_half_is_offered_where_a_description_can_be_shown() {
        for (sh, wants) in [("zsh", true), ("fish", true), ("bash", false)] {
            let text = script(sh).unwrap();
            assert_eq!(
                text.contains(crate::cli::COMPLETE_ARG),
                wants,
                "{sh} live completion is {}",
                if wants {
                    "missing"
                } else {
                    "offered and cannot show a label"
                }
            );
            if wants {
                assert!(
                    text.contains("2>/dev/null"),
                    "{sh} lets a failed read print into the command line"
                );
            }
        }
    }

    /// The finished zsh script dispatches the `ask`, `run` and `change`
    /// positionals to `_devplane_live`, none completes file names, and the
    /// helper is defined above the line zsh runs while loading the file.
    #[test]
    fn zsh_dispatches_the_live_positionals_to_the_helper() {
        let text = script("zsh").unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (name, values) in LIVE_POSITIONALS {
            let positionals: Vec<&&str> = lines
                .iter()
                .filter(|l| {
                    l.strip_prefix("':")
                        .and_then(|r| r.split([':', ' ']).next())
                        == Some(name)
                })
                .collect();
            assert!(
                !positionals.is_empty(),
                "no `{name}` positional in the script — the command tree renamed it"
            );
            for l in positionals {
                assert!(
                    l.ends_with(&format!(":_devplane_live {}' \\", values.name())),
                    "`{name}` still completes file names: {l}"
                );
            }
        }
        let helper = lines
            .iter()
            .position(|l| l.starts_with("_devplane_live()"))
            .expect("the helper is defined");
        let dispatch = lines
            .iter()
            .position(|l| l.starts_with(r#"if [ "$funcstack[1]" = "_devplane" ]"#))
            .expect("clap's own dispatch line");
        assert!(
            helper < dispatch,
            "the helper is defined after zsh has already run the completer"
        );
        // And nothing points at a command that does not exist.
        assert!(!text.contains("devplane-answer"), "{text}");
    }

    /// Pressing Tab starts nothing, and is silent when there is nothing to
    /// read: checked in the source and by running it against an empty home.
    #[tokio::test]
    async fn a_keystroke_never_starts_a_host() {
        // Production code only, and the forbidden call is assembled so this
        // guard does not match itself.
        let whole = include_str!("completions.rs");
        let code = whole.split("#[cfg(test)]").next().unwrap_or(whole);
        let forbidden = format!("connect_or{}(", "_start");
        assert!(
            !code.contains(&forbidden),
            "the completion path can start a host from a Tab press"
        );
        assert!(
            code.contains("Reader::open()"),
            "the completion path no longer reads through `Reader`, so this guards nothing"
        );

        // With `DEVPLANE_HOME` pointed at an empty directory there is no host
        // info to read, so `connect` fails and this must be silent.
        let empty =
            std::env::temp_dir().join(format!("dp-nohost-{}", uuid::Uuid::new_v4().simple()));
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
            "completing with no host was an error rather than silence"
        );
    }

    /// The budget is for a keystroke. Bounded on the constant rather than by
    /// timing a call, which would be flaky on a loaded machine.
    #[test]
    fn the_live_budget_is_a_keystroke() {
        assert!(
            LIVE_TIMEOUT <= Duration::from_millis(200),
            "{LIVE_TIMEOUT:?} is long enough for a shell to feel broken"
        );
        assert!(
            LIVE_TIMEOUT >= Duration::from_millis(50),
            "{LIVE_TIMEOUT:?} is short enough to lose a healthy host's answer"
        );
        // And the timeout is actually applied.
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

    /// An unknown value set is silence, because this runs on a keystroke.
    #[tokio::test]
    async fn an_unknown_value_set_says_nothing() {
        assert!(cmd_complete("nonsense").await.is_ok());
        assert_eq!(Values::parse("nonsense"), None);
        for v in Values::ALL {
            assert_eq!(
                Values::parse(v.name()),
                Some(*v),
                "`{}` round-trips",
                v.name()
            );
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

        let changes =
            serde_json::json!([{"id": "c-1", "title": "rate-limit login", "phase": "review"}]);
        assert_eq!(
            values(Values::Changes, &changes)[0],
            ("c-1".into(), Some("rate-limit login".into()))
        );
    }
}

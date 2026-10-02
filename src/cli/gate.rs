//! `devplane gate`: running this repository's own checks, and registering
//! them as a Spec Kit hook.

use crate::render;
use crate::render::{DIM, paint};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Runs this repository's own gates and reports what they exited with.
///
/// Decides on exit codes alone — no spec is opened, no prose graded — and
/// distinguishes "nothing was checked" from "the checks said no", because a
/// Spec Kit workflow reads the answer.
pub async fn cmd_gate_run(cwd: Option<PathBuf>, name: Option<String>, json: bool) -> Result<()> {
    use crate::core::change::GateState;

    let here = match cwd {
        Some(p) => p.canonicalize().context("that path does not exist")?,
        None => std::env::current_dir()?,
    };
    let root = crate::repo::find_repo_root(&here).unwrap_or(here);

    let config = match crate::core::ProjectConfig::load(&root) {
        Err(e) => {
            let state = GateState::ConfigUnreadable;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "state": state.as_str(),
                        "passed": state.passed(),
                        "summary": state.says(),
                        "error": e.to_string(),
                        "commands": [],
                    }))?
                );
            } else {
                println!("{}  devplane.toml", paint(render::RED, "unreadable"));
                println!();
                println!("  {}", paint(DIM, &e.to_string()));
                println!();
                println!("{}", state.says());
            }
            // Not a pass, and not a failure of the checks: they did not run.
            return Err(crate::cli::Exit(1).into());
        }
        Ok(c) => c,
    };

    // `check` always resolves; any other name must be declared, and an
    // unknown one is an error rather than a silent pass.
    let (label, commands, timeout) = match name.as_deref() {
        None => ("check", config.gates.check.clone(), config.gates.timeout),
        Some(n) => match config.gate_named(n) {
            Some((run, timeout)) => (n, run, timeout),
            None => {
                let declared: Vec<&str> = config.gates.named.keys().map(String::as_str).collect();
                let known = match declared.is_empty() {
                    true => "this repository declares no named gates".to_string(),
                    false => format!("declared here: {}", declared.join(", ")),
                };
                match json {
                    true => println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "state": "unknown_gate",
                            "passed": false,
                            "summary": format!("no gate named `{n}`"),
                            "declared": declared,
                            "commands": [],
                        }))?
                    ),
                    false => {
                        println!("{}  {n}", paint(render::RED, "no such gate"));
                        println!();
                        println!("  {}", paint(DIM, &known));
                    }
                }
                return Err(crate::cli::Exit(1).into());
            }
        },
    };

    if commands.is_empty() {
        let state = GateState::NoGates;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "state": state.as_str(),
                    "passed": state.passed(),
                    "summary": state.says(),
                    "commands": [],
                }))?
            );
        } else {
            println!("{}", paint(DIM, "no gates"));
            println!();
            println!("{}", state.says());
        }
        return Err(crate::cli::Exit(1).into());
    }

    // The person's own checkout: its caches are theirs, so nothing is redirected.
    let report = crate::gates::run(
        label,
        &commands,
        &root,
        timeout,
        1,
        &crate::gates::GateInput::default(),
    )
    .await;
    let state = match report.passed() {
        true => GateState::Passed,
        false => GateState::Failed,
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "gate": label,
                "state": state.as_str(),
                "passed": state.passed(),
                // The gate's own sentence names what failed; `says` frames it.
                "summary": report.summary(),
                "says": state.says(),
                "commands": report.commands,
            }))?
        );
    } else {
        for c in &report.commands {
            let (label, colour) = match c.passed() {
                true => ("ok", render::GREEN),
                false => ("failed", render::RED),
            };
            println!(
                "{}{}{}",
                render::pad(&paint(colour, label), 10),
                render::pad(&c.command, 24),
                paint(
                    DIM,
                    &format!("{} · {}", c.outcome.headline(), c.tests.says())
                )
            );
        }
        println!();
        println!("{}", report.summary());
        // No host and no change here, so nothing is written to the audit log;
        // `devplane change verify` is the recorded form.
        println!(
            "{}",
            paint(
                DIM,
                "Not recorded: this ran in a repository rather than against a change. \
                 `devplane change verify <id>` is the one that leaves a row."
            )
        );
    }

    match state.passed() {
        true => Ok(()),
        false => Err(crate::cli::Exit(1).into()),
    }
}

/// Registers Devplane's gate as a Spec Kit extension hook.
///
/// Writes `extensions.yml` only when it is absent. An existing file may carry
/// other hooks and comments that a round-trip would destroy, so the entry is
/// printed for the person to paste instead.
pub fn cmd_speckit_install(event: Option<String>, dry_run: bool, anyway: bool) -> Result<()> {
    use crate::spec::{DEFAULT_HOOK_EVENT, EXTENSIONS_FILE, HOOK_COMMAND, HOOK_EVENTS};

    let here = std::env::current_dir()?;
    let root = crate::repo::find_repo_root(&here).unwrap_or(here);

    if !root.join(".specify").is_dir() {
        anyhow::bail!("Spec Kit is not installed here — there is nothing to register with.");
    }

    // A hook with no gate behind it runs nothing and reports a failure every
    // time: a step in the workflow that can never pass.
    if !anyway {
        match crate::core::ProjectConfig::load(&root) {
            Ok(c) if c.has_gates() => {}
            Ok(_) => anyhow::bail!(
                "refused: {} declares no gate (`[gates] check`), so the hook would run nothing \
                 and report a failure every time. Declare the checks first — `devplane config \
                 check` reads back what is there — or pass `--anyway`.",
                root.join(crate::core::config::CONFIG_FILE).display()
            ),
            Err(e) => anyhow::bail!(
                "refused: {e}, so there is no gate for the hook to run. Fix it — `devplane \
                 config check` says where — or pass `--anyway`."
            ),
        }
    }

    let event = event.unwrap_or_else(|| DEFAULT_HOOK_EVENT.to_string());
    if !HOOK_EVENTS.contains(&event.as_str()) {
        anyhow::bail!(
            "`{event}` is not a hook point Spec Kit defines. It knows:\n  {}",
            HOOK_EVENTS.join("\n  ")
        );
    }

    // A mandatory hook whose skill the agent cannot reach is a workflow step
    // that can neither be skipped nor run, so it is refused before anything is
    // written.
    if !anyway && let Some(why) = crate::spec::unreachable_skill(&root, dirs::home_dir().as_deref())
    {
        let places = crate::spec::skill_locations(&root, dirs::home_dir().as_deref());
        anyhow::bail!(
            "{why}.\n\n\
             A hook registered `optional: false` is one the agent is told it may not skip. \n\
             Registering it now would put a step in the workflow that cannot run.\n\n\
             Put the `devplane-gate` skill (the plugin's `skills/devplane-gate/SKILL.md`) \
             at one of these places, where the agent looks for skills:\n      {}\n\n\
             Then run this again. `--anyway` writes it regardless.",
            places
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n      ")
        );
    }

    let path = root.join(EXTENSIONS_FILE);
    let existing = std::fs::read_to_string(&path).ok();

    // A text match, not a parse — which is also why the file is never edited.
    if let Some(text) = &existing
        && text.contains(HOOK_COMMAND)
    {
        println!(
            "{} already names `{HOOK_COMMAND}`. Nothing to do.",
            EXTENSIONS_FILE
        );
        return Ok(());
    }

    let entry = crate::spec::hook_entry();
    match existing {
        None => {
            println!("{EXTENSIONS_FILE} would be created, containing:\n");
            println!("{}", crate::spec::extensions_file(&event));
            if dry_run {
                println!(
                    "{}",
                    paint(DIM, "Nothing written. Run without --dry-run to add it.")
                );
                return Ok(());
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, crate::spec::extensions_file(&event))?;
            println!("{}  {EXTENSIONS_FILE}", paint(render::GREEN, "written"));
        }
        Some(_) => {
            // Never edited: see the doc comment.
            println!("{EXTENSIONS_FILE} exists, so add this under `hooks.{event}:`\n");
            println!("{entry}");
            println!(
                "{}",
                paint(
                    DIM,
                    "Devplane does not edit a file it has not parsed. Paste it, and commit it \
                     like the rest of the repository's configuration."
                )
            );
        }
    }
    Ok(())
}

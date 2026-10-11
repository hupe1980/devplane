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
    let report = crate::gates::run_in_turn(
        config.gates.serial,
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

/// The Spec Kit extension, as shipped in `extensions/speckit-devplane/`.
/// Embedded so installing it needs no download and always matches this binary.
const EXTENSION_FILES: &[(&str, &str)] = &[
    (
        "extension.yml",
        include_str!("../../extensions/speckit-devplane/extension.yml"),
    ),
    (
        "commands/gate.md",
        include_str!("../../extensions/speckit-devplane/commands/gate.md"),
    ),
    (
        "overlays/devplane-gate.yml",
        include_str!("../../extensions/speckit-devplane/overlays/devplane-gate.yml"),
    ),
    (
        "README.md",
        include_str!("../../extensions/speckit-devplane/README.md"),
    ),
    (
        "CHANGELOG.md",
        include_str!("../../extensions/speckit-devplane/CHANGELOG.md"),
    ),
    (
        "LICENSE-MIT",
        include_str!("../../extensions/speckit-devplane/LICENSE-MIT"),
    ),
    (
        "LICENSE-APACHE",
        include_str!("../../extensions/speckit-devplane/LICENSE-APACHE"),
    ),
];

/// The command the extension provides. Agents invoke it as
/// `/speckit-devplane-gate`.
const EXTENSION_COMMAND: &str = "speckit.devplane.gate";

/// Where Spec Kit puts the extension's overlay once it is installed.
const OVERLAY_INSTALLED: &str = ".specify/extensions/devplane/overlays/devplane-gate.yml";

/// The hook event the shipped manifest registers.
const MANIFEST_EVENT: &str = "after_implement";

/// A program on `PATH`, as a shell would find it.
fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let names: Vec<String> = match cfg!(windows) {
        true => vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            name.to_string(),
        ],
        false => vec![name.to_string()],
    };
    std::env::split_paths(&path)
        .find_map(|dir| names.iter().map(|n| dir.join(n)).find(|p| p.is_file()))
}

/// Whether `text` holds a hook entry whose `command` is exactly `command`.
/// A line match, because `speckit.devplane.gate` contains `devplane.gate`.
fn names_command(text: &str, command: &str) -> bool {
    text.lines().any(|l| {
        let l = l.trim().trim_start_matches("- ").trim();
        l.strip_prefix("command:")
            .map(|v| v.trim().trim_matches(['"', '\'']) == command)
            .unwrap_or(false)
    })
}

/// The extension's manifest with its hook moved to `event`.
fn manifest_for(event: &str) -> String {
    let manifest = EXTENSION_FILES[0].1;
    manifest.replacen(
        &format!("  {MANIFEST_EVENT}:\n"),
        &format!("  {event}:\n"),
        1,
    )
}

/// The entry Spec Kit adds to `extensions.yml` when it installs the extension.
fn extension_entry() -> String {
    format!(
        "    - extension: devplane\n\
         \x20     command: {EXTENSION_COMMAND}\n\
         \x20     description: Run this project's own gates and report what they said\n\
         \x20     prompt: Run the project's gates and report the verdict verbatim.\n\
         \x20     optional: false\n\
         \x20     enabled: true\n"
    )
}

/// The repository root for the working directory.
fn here_root() -> Result<PathBuf> {
    let here = std::env::current_dir()?;
    Ok(crate::repo::find_repo_root(&here).unwrap_or(here))
}

/// Refuses when the repository declares no gate to run.
fn require_gates(root: &std::path::Path, what: &str) -> Result<()> {
    match crate::core::ProjectConfig::load(root) {
        Ok(c) if c.has_gates() => Ok(()),
        Ok(_) => anyhow::bail!(
            "refused: {} declares no gate (`[gates] check`), so {what} would run nothing \
             and report a failure every time. Declare the checks first — `devplane config \
             check` reads back what is there — or pass `--anyway`.",
            root.join(crate::core::config::CONFIG_FILE).display()
        ),
        Err(e) => anyhow::bail!(
            "refused: {e}, so there is no gate for {what} to run. Fix it — `devplane \
             config check` says where — or pass `--anyway`."
        ),
    }
}

/// Installs Devplane's gate into this repository's Spec Kit.
///
/// With `specify` on `PATH`, Spec Kit's own `specify extension add` installs
/// the extension (command, hook, overlay) and `extensions.yml` is never
/// touched by hand. Without it, the hook entry is written only when the file
/// is absent; an existing file may carry other hooks and comments that a
/// round-trip would destroy, so the entry is printed for the person to paste.
pub fn cmd_speckit_install(event: Option<String>, dry_run: bool, anyway: bool) -> Result<()> {
    use crate::spec::{DEFAULT_HOOK_EVENT, EXTENSIONS_FILE, HOOK_EVENTS};

    let root = here_root()?;

    if !root.join(".specify").is_dir() {
        anyhow::bail!("Spec Kit is not installed here — there is nothing to register with.");
    }

    // A hook with no gate behind it runs nothing and reports a failure every
    // time: a step in the workflow that can never pass.
    if !anyway {
        require_gates(&root, "the hook")?;
    }

    let event = event.unwrap_or_else(|| DEFAULT_HOOK_EVENT.to_string());
    if !HOOK_EVENTS.contains(&event.as_str()) {
        anyhow::bail!(
            "`{event}` is not a hook point Spec Kit defines. It knows:\n  {}",
            HOOK_EVENTS.join("\n  ")
        );
    }

    let existing = std::fs::read_to_string(root.join(EXTENSIONS_FILE)).ok();
    match on_path("specify") {
        Some(specify) => {
            install_with_specify(&root, &specify, &event, existing.as_deref(), dry_run)
        }
        None => install_by_hand(&root, &event, existing, dry_run, anyway),
    }
}

/// Installs the extension through `specify extension add`, which owns
/// `extensions.yml`: Devplane prints what it will run and what Spec Kit will
/// add, then runs it.
fn install_with_specify(
    root: &std::path::Path,
    specify: &std::path::Path,
    event: &str,
    existing: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    use crate::spec::{EXTENSIONS_FILE, HOOK_COMMAND};

    if existing.is_some_and(|t| names_command(t, EXTENSION_COMMAND)) {
        println!(
            "{EXTENSIONS_FILE} already names `{EXTENSION_COMMAND}`: the Devplane extension is \
             installed. Nothing to do."
        );
        print_overlay_hint();
        return Ok(());
    }

    println!(
        "Spec Kit installs it, with its own command:\n\n  specify extension add --dev <the \
         Devplane extension, version {}>\n",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "That adds the command `{EXTENSION_COMMAND}` (in Claude Code, `/speckit-devplane-gate`) \
         and, under `hooks.{event}:` in {EXTENSIONS_FILE}:\n"
    );
    println!("{}", extension_entry());
    if existing.is_some_and(|t| names_command(t, HOOK_COMMAND)) {
        println!(
            "{}",
            paint(
                render::YELLOW,
                &format!(
                    "{EXTENSIONS_FILE} also carries a hand-written `{HOOK_COMMAND}` hook. With \
                     the extension installed the gate would run twice: remove that entry \
                     yourself — Devplane does not edit this file.\n"
                )
            )
        );
    }

    if dry_run {
        println!(
            "{}",
            paint(DIM, "Nothing run. Run without --dry-run to install it.")
        );
        return Ok(());
    }

    let package = std::env::temp_dir().join(format!(
        "devplane-speckit-extension-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    for (rel, text) in EXTENSION_FILES {
        let path = package.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = match *rel {
            "extension.yml" => manifest_for(event),
            _ => (*text).to_string(),
        };
        std::fs::write(&path, text)?;
    }

    let status = std::process::Command::new(specify)
        .args(["extension", "add", "--dev"])
        .arg(&package)
        .current_dir(root)
        .status()
        .with_context(|| format!("running {}", specify.display()));
    let _ = std::fs::remove_dir_all(&package);
    let status = status?;
    if !status.success() {
        anyhow::bail!(
            "`specify extension add` did not succeed ({status}); Devplane wrote nothing of its \
             own."
        );
    }
    println!();
    print_overlay_hint();
    Ok(())
}

/// The deterministic route, which is the person's to add: Spec Kit keeps
/// overlays in its own directory and Devplane does not put one there unasked.
fn print_overlay_hint() {
    println!(
        "The hook runs when the agent follows Spec Kit's hook text. For the workflow engine to \
         run the gate itself, after `implement` in the `speckit` workflow:\n\n  specify \
         workflow overlay add {OVERLAY_INSTALLED}"
    );
}

/// Without `specify`, the hook entry by hand: created when the file is
/// absent, printed to paste when it exists.
fn install_by_hand(
    root: &std::path::Path,
    event: &str,
    existing: Option<String>,
    dry_run: bool,
    anyway: bool,
) -> Result<()> {
    use crate::spec::{EXTENSIONS_FILE, HOOK_COMMAND};

    // A mandatory hook whose skill the agent cannot reach is a workflow step
    // that can neither be skipped nor run, so it is refused before anything is
    // written.
    if !anyway && let Some(why) = crate::spec::unreachable_skill(root, dirs::home_dir().as_deref())
    {
        let places = crate::spec::skill_locations(root, dirs::home_dir().as_deref());
        anyhow::bail!(
            "{why}.\n\n\
             A hook registered `optional: false` is one the agent is told it may not skip. \n\
             Registering it now would put a step in the workflow that cannot run.\n\n\
             Put the `devplane-gate` skill (the plugin's `skills/devplane-gate/SKILL.md`) \
             at one of these places, where the agent looks for skills:\n      {}\n\n\
             Or put Spec Kit's `specify` on PATH, and the extension brings its own command. \
             Then run this again. `--anyway` writes it regardless.",
            places
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n      ")
        );
    }

    let path = root.join(EXTENSIONS_FILE);

    // A text match, not a parse — which is also why the file is never edited.
    if let Some(text) = &existing
        && (names_command(text, HOOK_COMMAND) || names_command(text, EXTENSION_COMMAND))
    {
        println!("{EXTENSIONS_FILE} already registers Devplane's gate. Nothing to do.");
        return Ok(());
    }

    let entry = crate::spec::hook_entry();
    match existing {
        None => {
            println!("{EXTENSIONS_FILE} would be created, containing:\n");
            println!("{}", crate::spec::extensions_file(event));
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
            std::fs::write(&path, crate::spec::extensions_file(event))?;
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
    println!(
        "{}",
        paint(
            DIM,
            "Spec Kit's `specify` is not on PATH, so this is the hand-written hook. With it, \
             this command installs the Devplane extension instead."
        )
    );
    Ok(())
}

/// The name Claude Code gives a skill it runs before every commit.
pub const VERIFY_SKILL: &str = "verify";

/// The first line of the body: names who wrote the file and how to remove it.
const VERIFY_MARKER: &str =
    "<!-- Written by `devplane gate install --skill`. Delete this directory to remove it. -->";

/// The `verify` skill: the `devplane-gate` skill's instructions under the
/// name Claude Code runs before committing, framed as the agent's own run.
#[must_use]
pub fn verify_skill() -> String {
    let gate = include_str!("../../plugin/skills/devplane-gate/SKILL.md");
    // Everything after the gate skill's frontmatter.
    let body = gate
        .strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---\n"))
        .map_or(gate, |(_, body)| body)
        .trim_start();
    format!(
        "---\n\
         name: {VERIFY_SKILL}\n\
         description: Run this repository's own checks (`devplane gate`) and report what they \
         exited with. Use right before committing, and whenever asked whether this project's \
         checks pass. Reports only; never fixes, never re-runs, and never calls a pass verified.\n\
         ---\n\
         \n\
         {VERIFY_MARKER}\n\
         \n\
         This is **your own run** of the project's gates, before a commit. Report it as *passed* or \
         *failed* — never *verified*: that word belongs to a check Devplane runs against a recorded \
         tree, and this is not one. If it did not pass, say so before committing and let the person \
         decide.\n\
         \n\
         {body}"
    )
}

/// Offers the gate as the skill `verify`, shown as a diff and written only on
/// confirmation.
pub fn cmd_skill_install(
    user: bool,
    yes: bool,
    dry_run: bool,
    anyway: bool,
    json: bool,
) -> Result<()> {
    let root = here_root()?;
    let base = match user {
        true => dirs::home_dir().context("no home directory to put a user skill in")?,
        false => root.clone(),
    };
    let path = base
        .join(".claude/skills")
        .join(VERIFY_SKILL)
        .join("SKILL.md");

    // A repository-scoped skill with no gate behind it reports a failure
    // before every commit. A user-level one is for every repository, and
    // says plainly where none is declared.
    if !user && !anyway {
        require_gates(&root, "the skill")?;
    }

    let after = verify_skill();
    let before = std::fs::read_to_string(&path).ok();
    if before.as_deref() == Some(after.as_str()) {
        match json {
            true => println!(
                "{}",
                serde_json::json!({ "path": path, "written": false, "unchanged": true })
            ),
            false => println!("{} is already this skill. Nothing to do.", path.display()),
        }
        return Ok(());
    }
    // Somebody's own `verify` skill is not replaced on a flag alone.
    let foreign = before
        .as_deref()
        .is_some_and(|b| !b.contains(VERIFY_MARKER));
    if foreign && !anyway {
        anyhow::bail!(
            "{} is a `verify` skill Devplane did not write, so it is left alone. Read it, and \
             pass `--anyway` to be offered the replacement.",
            path.display()
        );
    }
    let diff = crate::observe::connect::diff(before.as_deref().unwrap_or(""), &after);

    if json {
        let written = !dry_run && yes;
        if !dry_run && !yes {
            anyhow::bail!(
                "nothing was written to {}: under --json nobody can confirm the change. Re-run \
                 with `--yes` to write it (the diff is shown without --json).",
                path.display()
            );
        }
        if written {
            write_skill(&path, &after)?;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": path,
                "diff": diff,
                "written": written,
            }))?
        );
        return Ok(());
    }

    println!(
        "Claude Code runs a skill named `{VERIFY_SKILL}` right before it commits. This one runs \
         `devplane gate` and reports what the project's own checks exited with.\n"
    );
    println!("{} {}", paint(render::BOLD, "Changes to"), path.display());
    for line in diff.lines() {
        let painted = match line.as_bytes().first() {
            Some(b'+') => paint(render::GREEN, line),
            Some(b'-') => paint(render::RED, line),
            _ => paint(DIM, line),
        };
        println!("  {painted}");
    }
    println!();
    if dry_run {
        println!(
            "{}",
            paint(DIM, "Nothing written. Run without --dry-run to be asked.")
        );
        return Ok(());
    }
    if !super::confirm("Write this?", yes)? {
        println!("{}", paint(DIM, "Nothing written."));
        return Ok(());
    }
    write_skill(&path, &after)?;
    println!("{}  {}", paint(render::GREEN, "written"), path.display());
    Ok(())
}

fn write_skill(path: &std::path::Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_command_is_not_mistaken_for_the_hand_written_one() {
        let ext = "hooks:\n  after_implement:\n  - extension: devplane\n    command: speckit.devplane.gate\n";
        assert!(names_command(ext, EXTENSION_COMMAND));
        assert!(!names_command(ext, crate::spec::HOOK_COMMAND));
        let hand = r#"hooks:
  after_implement:
    - extension: devplane
      command: devplane.gate
"#;
        assert!(names_command(hand, crate::spec::HOOK_COMMAND));
        assert!(!names_command(hand, EXTENSION_COMMAND));
    }

    #[test]
    fn the_embedded_manifest_registers_the_gate_mandatory_and_without_a_condition() {
        let m = EXTENSION_FILES[0].1;
        assert!(m.contains("id: \"devplane\""), "{m}");
        assert!(m.contains(&format!("name: \"{EXTENSION_COMMAND}\"")), "{m}");
        assert!(
            m.contains(&format!("command: \"{EXTENSION_COMMAND}\"")),
            "{m}"
        );
        assert!(m.contains("optional: false"), "{m}");
        // Comments may mention it; no key may set it.
        assert!(
            !m.lines().any(|l| l.trim_start().starts_with("condition:")),
            "a condition makes every command skip the hook"
        );
        assert!(m.contains(&format!("  {MANIFEST_EVENT}:\n")));
        let moved = manifest_for("after_converge");
        assert!(moved.contains("  after_converge:\n") && !moved.contains("  after_implement:\n"));
    }

    #[test]
    fn the_overlay_runs_the_gate_as_a_shell_step_after_implement() {
        let o = EXTENSION_FILES[2].1;
        assert!(o.contains("extends: \"speckit\""), "{o}");
        assert!(o.contains("insert_after: implement"), "{o}");
        assert!(o.contains("type: shell"), "{o}");
        assert!(o.contains("run: \"devplane gate --json\""), "{o}");
        // A gate that fails must end the run: nothing may swallow it.
        assert!(!o.contains("continue_on_error"), "{o}");
    }

    #[test]
    fn the_verify_skill_is_named_verify_marked_and_never_says_verified_of_its_pass() {
        let s = verify_skill();
        assert!(s.starts_with("---\nname: verify\ndescription: "), "{s}");
        assert!(s.contains(VERIFY_MARKER));
        assert!(s.contains("devplane gate --json"));
        assert!(s.contains("never *verified*"));
        // The gate skill's frontmatter is not carried into the body.
        assert!(!s.contains("name: devplane-gate"));
    }
}

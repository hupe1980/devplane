//! Change: starting it, checking a project's configuration, and reading a verdict.

use super::ChangeCmd;
use crate::core::change::{ChangeState, Standing};
use crate::render::{BOLD, DIM, YELLOW, clip, paint};
use crate::{client, render};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Where the rules governing a directory came from, and whether they loaded.
/// Kept apart so a typo in a deny rule never reads as "no rule answers this".
enum Rules {
    /// No `devplane.toml` governs this directory at all.
    None { root: PathBuf },
    /// One was found and will not load, so *nothing* is in force here.
    Broken { path: PathBuf, error: String },
    /// Loaded. `problems` are the rules that cannot do what they say.
    Loaded {
        path: PathBuf,
        count: usize,
        problems: Vec<crate::core::config::Problem>,
        /// Rules that provably do nothing (`Policy::redundancies`), carried so
        /// `--json` reports them too.
        unused: Vec<String>,
        /// Allow rules that grant an arbitrary program (`Policy::overbroad`).
        overbroad: Vec<crate::core::policy::Overbroad>,
    },
}

impl Rules {
    fn at(dir: &Path) -> Self {
        let root = crate::repo::governing_root(dir).unwrap_or_else(|| dir.to_path_buf());
        let path = root.join(crate::core::config::CONFIG_FILE);
        match crate::core::ProjectConfig::load(&root) {
            Err(e) => Rules::Broken {
                path,
                error: e.to_string(),
            },
            Ok(_) if !path.is_file() => Rules::None { root },
            Ok(c) => {
                let p = c.policy();
                Rules::Loaded {
                    count: p.deny_rules().len() + p.ask_rules().len(),
                    problems: c.validate(),
                    unused: p.redundancies(),
                    overbroad: crate::core::policy::overbroad(&vendor_allow_rules(root.as_path())),
                    path,
                }
            }
        }
    }

    fn problems(&self) -> Vec<crate::core::config::Problem> {
        match self {
            Rules::Loaded { problems, .. } => problems.clone(),
            // A file that will not load has no problems to list; `load_error`
            // reports that case instead of "0 problems".
            _ => Vec::new(),
        }
    }

    fn load_error(&self) -> Option<String> {
        match self {
            Rules::Broken { error, .. } => Some(error.clone()),
            _ => None,
        }
    }

    /// The sentence printed when no rule answered — one per situation.
    fn why_nothing_answered(&self) -> &'static str {
        match self {
            Rules::None { .. } => {
                "no devplane.toml governs this directory, so nothing here decides anything"
            }
            Rules::Broken { .. } => {
                "this project's rules are NOT in force — the file below will not load"
            }
            Rules::Loaded { count: 0, .. } => {
                "this project's devplane.toml declares no [policy] rules"
            }
            Rules::Loaded { .. } => {
                "its rules loaded and none answers this one, so the provider's own \
                 dialog decides and it reaches your inbox"
            }
        }
    }

    fn as_json(&self) -> serde_json::Value {
        match self {
            Rules::None { root } => serde_json::json!({
                "state": "none", "searched_from": root.display().to_string()
            }),
            Rules::Broken { path, error } => serde_json::json!({
                "state": "broken", "path": path.display().to_string(), "error": error,
                "in_force": false
            }),
            Rules::Loaded {
                path,
                count,
                problems,
                unused,
                overbroad,
            } => serde_json::json!({
                "state": "loaded", "path": path.display().to_string(),
                "rules": count,
                "problems": problems.iter().map(|p| format!("{}: {}", p.where_, p.what)).collect::<Vec<_>>(),
                "unused": unused,
                "overbroad": overbroad.iter().map(|o| serde_json::json!({
                    "rule": o.rule, "why": o.why, "suggestion": o.suggestion
                })).collect::<Vec<_>>(),
                "in_force": true
            }),
        }
    }
}

/// `devplane explain` — what the gate would decide about one call, and why.
///
/// Offline and host-free, so rules can be tested while being written and the
/// answer is scriptable. The interesting output is an `undecided` that looks
/// like an allow: a rule can match and still not answer.
pub fn cmd_explain(
    path: PathBuf,
    tool: String,
    call: Vec<String>,
    input: Option<String>,
    json: bool,
) -> Result<()> {
    let dir = path.canonicalize().context("that path does not exist")?;
    let input: serde_json::Value = match input {
        Some(raw) => serde_json::from_str(&raw).context("--input is not JSON")?,
        None => {
            let text = call.join(" ");
            if text.is_empty() {
                anyhow::bail!("say what to explain: a command, or --input '{{…}}'");
            }
            match crate::core::policy::rule_content_field(&tool) {
                Some(field) => serde_json::json!({ field: text }),
                None => anyhow::bail!(
                    "`{tool}` takes no plain specifier, so pass the call as --input '{{…}}'"
                ),
            }
        }
    };

    // The machine-wide policy is included: this is the gate as enforced.
    let (cache, global_error) = crate::policy_cache::PolicyCache::from_disk();
    let verdict = cache.restrictive(&dir, &tool, &input);

    // Where the rules came from and whether they loaded: a broken
    // `devplane.toml` has no rules in force, and must not read as "no rule
    // answers this one".
    let rules = Rules::at(&dir);
    let problems = rules.problems();

    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "dir": dir.display().to_string(),
                "tool": tool,
                "input": input,
                "verdict": verdict.as_str(),
                "rule": verdict.rule(),
                // Set only for `unresolved`, where there is no rule to name.
                "why": verdict.why(),
                // Which `undecided` situation this is.
                "rules": rules.as_json(),
            }))?
        );
        return Ok(());
    }

    let (colour, headline) = match &verdict {
        crate::core::Verdict::Deny { .. } => (render::RED, "deny"),
        crate::core::Verdict::Ask { .. } => (YELLOW, "ask"),
        crate::core::Verdict::Unresolved { .. } => (YELLOW, "ask — unreadable"),
        crate::core::Verdict::Undecided => (DIM, "undecided"),
    };
    println!("{}  {}", paint(colour, headline), paint(BOLD, &tool));
    match (verdict.rule(), verdict.why()) {
        (Some(r), _) => println!("        {}", paint(DIM, &format!("by {r}"))),
        // No rule, but a reason: the matcher could not read the command, so
        // "no rule covers this" would be false.
        (None, Some(why)) => println!("        {}", paint(DIM, &format!("because {why}"))),
        (None, None) => println!("        {}", paint(DIM, rules.why_nothing_answered())),
    }
    if let Some(e) = &global_error {
        println!(
            "\n{}\n{}",
            paint(
                render::RED,
                "the machine-wide rules would not load, so every call is asked"
            ),
            paint(DIM, e)
        );
    }
    if let Some(broken) = rules.load_error() {
        println!("\n{}", paint(render::RED, &broken));
        println!(
            "{}",
            paint(
                DIM,
                "every call here is asked until it parses — devplane check"
            )
        );
    }
    if !problems.is_empty() {
        println!(
            "\n{}",
            paint(
                YELLOW,
                &format!(
                    "{} rule(s) in this project cannot do what they say — devplane check",
                    problems.len()
                )
            )
        );
    }
    Ok(())
}

/// Replay every tool call this machine has seen against the current rules,
/// and suggest the rules that would stop repeated prompts.
///
/// Offline: opens the store read-only and asks no agent anything.
pub async fn cmd_replay(dir: PathBuf, limit: i64, json: bool) -> Result<()> {
    let dir = dir.canonicalize().context("that path does not exist")?;
    let store = crate::store::Store::open(&crate::config::db_path()?).await?;
    // Scoped to the repository root containing `--dir`, so a session started
    // in `src/` counts toward the same project.
    let scope = crate::repo::find_repo_root(&dir).unwrap_or_else(|| dir.clone());
    let named = scope
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| scope.display().to_string());
    let calls = store.observed_tool_calls(Some(&scope), limit).await?;

    // Includes the machine-wide policy, as the gate does.
    let (cache, _) = crate::policy_cache::PolicyCache::from_disk();
    // The third bucket is "no rule here decides" — not "interrupted a person":
    // the agent's own settings answer most of these silently.
    let mut counts = [0usize; 3]; // ask, deny, no rule here
    // Undecided commands by rule family; `BTreeMap` for stable output.
    let mut open: std::collections::BTreeMap<String, Vec<String>> = Default::default();

    for c in &calls {
        let verdict = cache.restrictive(&c.cwd, &c.tool, &c.input);
        let slot = match verdict {
            crate::core::Verdict::Ask { .. } | crate::core::Verdict::Unresolved { .. } => 0,
            crate::core::Verdict::Deny { .. } => 1,
            crate::core::Verdict::Undecided => 2,
        };
        counts[slot] += 1;
        if slot != 2 {
            continue;
        }
        let Some(text) = crate::core::policy::rule_content(&c.tool, &c.input) else {
            continue;
        };
        // Grouped by the family a rule would name (program and subcommand),
        // not by tool, so `suggest_rule_for` is not asked to cover unrelated
        // commands in one rule.
        let family = crate::core::command::rule_family(&c.tool, &text);
        open.entry(format!("{}\u{0}{}", c.tool, family))
            .or_default()
            .push(text);
    }

    // A rule is suggested only for a prompt that keeps coming back: below
    // three, a standing grant costs more than the interruption it saves.
    const WORTH_A_RULE: usize = 3;
    let mut advice: Vec<(usize, String, String)> = Vec::new();
    for (key, commands) in &open {
        let (tool, _) = key.split_once('\u{0}').unwrap_or((key.as_str(), ""));
        if commands.len() < WORTH_A_RULE {
            continue;
        }
        if let Some(sp) = crate::core::command::suggest_rule_for(tool, commands) {
            advice.push((commands.len(), tool.to_string(), sp));
        }
    }
    advice.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)));

    let total = calls.len();
    let answerable: usize = advice.iter().map(|a| a.0).sum();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "dir": scope.display().to_string(),
                "calls": total,
                "ask": counts[0], "deny": counts[1], "undecided": counts[2],
                "suggestions": advice.iter().map(|(n, tool, rule)| serde_json::json!({
                    "calls": n, "tool": tool, "rule": format!("{tool}({rule})"),
                })).collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }

    if total == 0 {
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "no tool calls observed in {named} — `devplane connect claude`, then come back"
                )
            )
        );
        return Ok(());
    }

    let pct = |n: usize| (n as f64 * 100.0 / total as f64).round() as u64;
    println!("{total} tool calls in {named} replayed against the rules as they are now\n");
    for (n, label, colour) in [
        (counts[0], "ask", YELLOW),
        (counts[1], "deny", render::RED),
        (counts[2], "no rule here", DIM),
    ] {
        if n > 0 {
            println!("  {:>6}  {:>3}%  {}", n, pct(n), paint(colour, label));
        }
    }

    if advice.is_empty() {
        println!("\n{}", paint(DIM, "nothing here has a rule worth writing"));
        return Ok(());
    }

    const SHOWN: usize = 12;
    println!(
        "\n{}",
        paint(BOLD, "one rule each, most interruptions first")
    );
    for (n, tool, rule) in advice.iter().take(SHOWN) {
        println!(
            "  {:>5}×  {}",
            n,
            paint(render::GREEN, &format!("{tool}({rule})"))
        );
    }
    if advice.len() > SHOWN {
        println!(
            "  {}",
            paint(DIM, &format!("… and {} more", advice.len() - SHOWN))
        );
    }
    let shown: usize = advice.iter().take(SHOWN).map(|a| a.0).sum();
    println!(
        "\n{}",
        paint(
            DIM,
            &format!(
                "{shown} of the {} calls Devplane leaves to your agent · \
                 paste into permissions.allow in your agent's settings",
                counts[2]
            )
        )
    );
    let _ = answerable;
    Ok(())
}

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
    let report = crate::gates::run(label, &commands, &root, timeout, 1, &[]).await;
    let state = match report.passed() {
        true => GateState::Verified,
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
                paint(DIM, &c.outcome.headline())
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

pub async fn cmd_check(path: PathBuf, json: bool) -> Result<()> {
    let root = path.canonicalize().context("that path does not exist")?;
    let root = crate::repo::find_repo_root(&root).unwrap_or(root);
    let file = root.join(crate::core::config::CONFIG_FILE);

    let config = match crate::core::ProjectConfig::load(&root) {
        Ok(c) => c,
        Err(e) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({ "ok": false, "error": e.to_string() })
                    )?
                );
                return Err(crate::cli::Exit(1).into());
            }
            anyhow::bail!("{e}");
        }
    };
    // Validated against registered projects when a store exists, so an
    // unknown `deliver_from` is named. No store is created here.
    let problems = match registered_names().await {
        Some(names) => config.validate_against(&names),
        None => config.validate(),
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "file": file.display().to_string(),
                "exists": file.exists(),
                "ok": !problems.iter().any(|p| p.fatal),
                // The same read-back the board shows; it carries `problems`.
                "describes": config.describe(),
            }))?
        );
        if problems.iter().any(|p| p.fatal) {
            return Err(crate::cli::Exit(1).into());
        }
        return Ok(());
    }

    if !file.exists() {
        println!(
            "{}\n\nNothing is verified and nothing pretends to be. \
             Add a devplane.toml when you want gates.",
            paint(DIM, &format!("no {}", file.display()))
        );
        // The specifications are read without one: the layout is the tool's own.
        for layout in &config.spec.layouts(&root) {
            let n = crate::spec::changes(&root, layout).len();
            println!(
                "\n  {:<9} {} in {}/ ({n} found, by {})",
                "spec", layout.name, layout.root, layout.detected_by
            );
        }
        return Ok(());
    }

    for p in &problems {
        println!(
            "{} {} {}",
            match p.fatal {
                true => paint(render::RED, "error  "),
                false => paint(render::YELLOW, "warning"),
            },
            paint(BOLD, &p.where_),
            p.what
        );
    }
    if problems.iter().any(|p| p.fatal) {
        println!();
        anyhow::bail!("{} cannot do what it says", file.display());
    }

    println!("{}", paint(render::GREEN, "ok"));
    println!(
        "  gates     {}",
        match config.gates.check.is_empty() {
            true => paint(DIM, "none — nothing is verified"),
            false => config.gates.check.join(" && "),
        }
    );
    for (name, gate) in &config.gates.named {
        println!("            {name}: {}", gate.run.join(" && "));
    }
    println!(
        "  pull req  {}",
        match config.github.pull_request {
            true if config.github.draft => "yes, as a draft",
            true => "yes",
            false => "no",
        }
    );
    // What happens to an unanswered question. An unparsable value is already
    // listed as a problem above, so it is not repeated here.
    if let Some(d) = config.questions.deadline() {
        println!("  questions {}", crate::core::ask::Deadline::says(d));
    }

    // Every specification layout found — by the tool's own marker, or the
    // `[spec] plans` key — with how many changes each holds; or the sentence
    // naming what would have been recognised.
    let layouts = config.spec.layouts(&root);
    if layouts.is_empty() {
        println!("  {:<9} {}", "spec", crate::spec::NO_LAYOUT);
    }
    for layout in &layouts {
        let n = crate::spec::changes(&root, layout).len();
        println!(
            "  {:<9} {} in {}/ ({n} found, by {})",
            "spec", layout.name, layout.root, layout.detected_by
        );
    }
    if !config.spec.open_questions.is_empty() {
        // Padded by width, not spaces in the literal: `purity` rejects runs of
        // spaces in strings as a dropped `\` continuation.
        println!(
            "  {:<9} unresolved when a line says: {}",
            "spec",
            config.spec.open_questions.join(", ")
        );
    }

    // What the rules actually do: a rule can parse and still cover nothing
    // anybody expected.
    let policy = config.policy();
    if !policy.is_empty() {
        println!(
            "  policy    {} deny, {} ask",
            policy.deny_rules().len(),
            policy.ask_rules().len()
        );
        // Printed in evaluation order. A negation is labelled `except`, since
        // `deny  !Bash(git status)` would otherwise read as its opposite.
        let badge = |rule: &crate::core::policy::Rule, colour: &'static str, word: &'static str| {
            if rule.is_negated() {
                paint(render::DIM, "except")
            } else {
                paint(colour, word)
            }
        };
        for rule in policy.deny_rules() {
            println!(
                "            {} {}",
                badge(rule, render::RED, "deny  "),
                rule
            );
        }
        for rule in policy.ask_rules() {
            println!(
                "            {} {}",
                badge(rule, render::YELLOW, "ask   "),
                rule
            );
        }
        for finding in &policy.redundancies() {
            let mut lines = crate::core::text::wrap(finding, 66).into_iter();
            if let Some(first) = lines.next() {
                println!("  {}{}", render::pad(&paint(BOLD, "unused"), 10), first);
            }
            for rest in lines {
                println!("  {:<10}{}", "", rest);
            }
        }
        // Allow rules that provably grant more than they read as granting
        // (pattern containment, not a heuristic).
        let wide = crate::core::policy::overbroad(&vendor_allow_rules(root.as_path()));
        if !wide.is_empty() {
            println!();
        }
        for finding in &wide {
            println!(
                "  {}{}",
                render::pad(&paint(BOLD, "overbroad"), 10),
                finding.rule
            );
            for line in crate::core::text::wrap(&finding.why, 66) {
                println!("  {:<10}{}", "", line);
            }
            println!(
                "  {:<10}{}",
                "",
                paint(DIM, &format!("narrow it, e.g. {}", finding.suggestion))
            );
        }

        let half = policy.half_protected_paths();
        if !half.is_empty() {
            let add = half
                .iter()
                .map(|p| format!("\"Edit({p})\""))
                .collect::<Vec<_>>()
                .join(", ");
            let first = &half[0];
            println!(
                "\n  {:<10}those denies reach reads and the operands of writers — a redirection like `echo x > {first}` still runs",
                paint(BOLD, "note")
            );
            println!(
                "  {:<10}{}",
                "",
                paint(DIM, &format!("add {add} to never_auto to stop writes too"))
            );
        }
    }
    if !problems.is_empty() {
        println!();
    }
    Ok(())
}

/// `devplane trust` — show what a repository's agent configuration does, then
/// let a person decide.
///
/// Reports and refuses nothing, prints nothing extra for a repository with
/// none of it, and `--dry-run` trusts nothing.
pub async fn cmd_trust(path: PathBuf, yes: bool, dry_run: bool, json: bool) -> Result<()> {
    let root = path.canonicalize().context("that path does not exist")?;
    let setup = crate::setup::scan(&root);

    if json {
        let findings: Vec<serde_json::Value> = setup
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "kind": f.kind.as_str(), "source": f.source,
                    "subject": f.subject, "detail": f.detail
                })
            })
            .collect();
        if dry_run {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "path": root.display().to_string(),
                    "trusted": false,
                    "findings": findings,
                    // Zero findings does not mean the repository ships nothing.
                    "skills": setup.skills,
                    "unreadable": setup.unreadable,
                }))?
            );
            return Ok(());
        }
        let v = trust_call(&root).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": root.display().to_string(),
                "trusted": v.get("error").is_none(),
                "error": v.get("error"),
                "findings": findings,
                "skills": setup.skills,
                "unreadable": setup.unreadable,
            }))?
        );
        return Ok(());
    }

    print_setup(&root, &setup);

    if dry_run {
        println!("  {}", paint(DIM, "nothing was trusted (--dry-run)"));
        return Ok(());
    }
    // Nothing to show means nothing to confirm.
    if !setup.is_empty() && !crate::cli::confirm("Trust this repository?", yes)? {
        println!("  {}", paint(DIM, "not trusted"));
        return Ok(());
    }
    let v = trust_call(&root).await?;
    match v.get("error").and_then(|e| e.as_str()) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!(
            "{} {}\n  {}",
            paint(render::GREEN, "trusted"),
            root.display(),
            paint(DIM, "agents may now be started here")
        ),
    }
    Ok(())
}

async fn trust_call(root: &Path) -> Result<serde_json::Value> {
    // Through the host where one runs, so its world hears it at once;
    // otherwise straight into the store, which a host reads when it starts.
    // Trusting is a person's record, and needs nothing running.
    if let Ok(c) = client::Client::connect_running().await {
        return c
            .post_json(
                "/api/projects/trust",
                &serde_json::json!({ "path": root.to_string_lossy() }),
            )
            .await;
    }
    let root = root
        .canonicalize()
        .with_context(|| format!("{} does not exist", root.display()))?;
    let store = crate::store::Store::open(&crate::config::db_path()?)
        .await
        .context("opening the store")?;
    let mut project = crate::core::Project::from_root(root);
    project.trusted = true;
    store.save_project(&project).await.with_context(|| {
        format!(
            "could not record that {} is trusted, so it is not",
            project.root.display()
        )
    })?;
    Ok(serde_json::json!({ "trusted": true, "project": project }))
}

/// What starting an agent here will load, grouped the way a person reads it.
fn print_setup(root: &Path, setup: &crate::setup::Setup) {
    if setup.is_empty() {
        // Findings are what is worth flagging; skills are what is there.
        let quiet = match setup.skills {
            0 => "declares no hooks, MCP servers or skills".to_string(),
            1 => "declares 1 skill, and no hooks, MCP servers or pre-approved tools".to_string(),
            n => format!("declares {n} skills, and no hooks, MCP servers or pre-approved tools"),
        };
        println!("  {}", paint(DIM, &format!("{} {quiet}", root.display())));
        return;
    }
    println!(
        "  {}",
        paint(DIM, "starting an agent here loads this repository's own:")
    );
    println!();
    for f in &setup.findings {
        println!(
            "  {:<8}{}",
            paint(BOLD, f.kind.as_str()),
            crate::core::text::clip(&f.subject, 66)
        );
        for line in crate::core::text::wrap(&f.detail, 66) {
            println!("  {:<8}{}", "", paint(DIM, &line));
        }
    }
    for u in &setup.unreadable {
        println!(
            "  {:<8}{}",
            paint(BOLD, "unread"),
            format_args!("{u} exists and could not be parsed")
        );
        println!(
            "  {:<8}{}",
            "",
            paint(DIM, "the agent will read it and this could not")
        );
    }
    println!();
}

/// The names of every project this machine has registered, or nothing where
/// there is no store yet.
async fn registered_names() -> Option<Vec<String>> {
    let db = crate::config::db_path().ok()?;
    if !db.exists() {
        return None;
    }
    let store = crate::store::Store::open(&db).await.ok()?;
    let projects = store.load_projects().await.ok()?;
    Some(projects.into_iter().map(|p| p.name).collect())
}

/// The review, worded exactly as the surface words it: every sentence is the
/// host's, and this only lays them out.
fn print_review(r: &crate::view::ReviewView, by_intent: bool) {
    println!(
        "{} {}",
        paint(BOLD, &r.title),
        paint(DIM, &format!("against {}", r.base))
    );
    println!("  {}", r.shape_says);
    println!("  {}", r.standing_says);
    for s in [
        &r.unordered,
        &r.coverage_absent,
        &r.truncated_says,
        &r.empty_says,
    ]
    .into_iter()
    .flatten()
    {
        println!("  {}", paint(YELLOW, s));
    }
    let row = |f: &crate::view::ReviewFile| {
        let mut cells = vec![f.path.clone(), f.role_says.clone()];
        if let Some(c) = &f.coverage_says {
            cells.push(c.clone());
        }
        cells.push(match f.decisions.len() {
            0 => "no decision recorded".to_string(),
            1 => "1 decision".to_string(),
            n => format!("{n} decisions"),
        });
        if !f.task_says.is_empty() {
            cells.push(f.task_says.clone());
        }
        println!("    {}", cells.join(" · "));
    };
    if by_intent {
        println!();
        println!("{}", paint(BOLD, &r.intent.heading));
        if let Some(why) = &r.intent.unavailable {
            println!("  {}", paint(YELLOW, why));
        }
        for g in &r.intent.groups {
            println!("  {}", g.title);
            println!("    {}", paint(DIM, &g.says));
            for f in &g.files {
                println!("    {f}");
            }
        }
        if !r.intent.not_asked_for.is_empty() {
            println!("  {}", paint(YELLOW, &r.intent.not_asked_for_heading));
            for n in &r.intent.not_asked_for {
                println!("    {} — {}", n.path, n.says);
            }
        }
    } else {
        for g in &r.groups {
            println!();
            println!("  {}", paint(BOLD, &g.says));
            // The weakened rows, each with what it matched and whether a
            // person marked it seen; then the files.
            for w in &g.weakened {
                println!(
                    "    {} — {} {}",
                    w.row.path,
                    paint(YELLOW, &w.row.why),
                    paint(
                        DIM,
                        match w.seen {
                            true => "(seen)",
                            false => "(unseen)",
                        }
                    )
                );
                println!("      {}", paint(DIM, &clip(&w.row.matched, 120)));
            }
            for f in &g.files {
                row(f);
            }
        }
    }
    if let Some(s) = &r.formatter_only_says {
        println!();
        println!("  {}", paint(DIM, s));
    }
}

/// Refuses a decision about a change from inside an agent's session: an agent
/// may not offer, finish, archive or mark read its own work. The same check
/// `answer` makes, with the same limit: it narrows the hole and does not close
/// it — an agent that unsets the variable, or calls the host with the token in
/// `~/.devplane/token`, is not stopped by it.
fn refuse_from_agent(what: &str) -> Result<()> {
    if let Some(var) = crate::hook::agent_session() {
        anyhow::bail!(
            "refused: `{var}` is set, so this is running inside an agent's session, and an \
             agent may not {what} its own change. Do it from the Devplane window or a terminal \
             of your own. This check narrows the hole and does not close it: an agent that \
             unsets the variable, or calls the host with its token, is not stopped by it."
        );
    }
    Ok(())
}

/// The exit status of a refusal, distinct from an error's 1.
const REFUSED: i32 = 3;

pub async fn cmd_change(what: ChangeCmd, json: bool) -> Result<()> {
    // Decisions a person takes, refused from an agent's session before
    // anything is read.
    match &what {
        ChangeCmd::Offer { .. } => refuse_from_agent("offer")?,
        ChangeCmd::Finish { .. } => refuse_from_agent("finish")?,
        ChangeCmd::Archive { .. } => refuse_from_agent("archive")?,
        ChangeCmd::Review { seen, .. } if !seen.is_empty() => {
            refuse_from_agent("mark read a weakened check of")?
        }
        _ => {}
    }
    // Reads work from the store; starting, steering or finishing needs a host.
    let host = client::Client::connect_running().await;
    match what {
        ChangeCmd::Start {
            title,
            agent,
            project,
            no_worktree,
            issue,
            spec,
            task,
        } => {
            let c = host?;
            // Also refused by the parser; repeated for hand-built command lines.
            if !task.is_empty() && spec.is_none() {
                anyhow::bail!("--task needs --spec: a task is a line in the folder --spec names");
            }
            let projects = match project.is_empty() {
                true => vec![std::env::current_dir()?.to_string_lossy().to_string()],
                false => project,
            };
            let title = title.join(" ");
            if title.is_empty() && issue.is_none() {
                anyhow::bail!(
                    "say what the change is: devplane change start \"fix the flaky test\"\n\
                     or point at an issue:  devplane change start --issue 7"
                );
            }
            let body = serde_json::json!({
                "projects": projects,
                "title": title,
                "agent": agent,
                "worktree": !no_worktree,
                "issue": issue,
                "spec": spec,
                "tasks": task,
            });
            // Every refusal, and what the first run will cost, before anything
            // is written.
            let pre: serde_json::Value = c.post_json("/api/changes/preflight", &body).await?;
            if let Some(e) = pre.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            let empty = vec![];
            let targets = pre["targets"].as_array().unwrap_or(&empty);
            let refused: Vec<&str> = targets.iter().filter_map(|t| t["says"].as_str()).collect();
            if !refused.is_empty() {
                if json {
                    println!("{}", serde_json::to_string_pretty(&pre)?);
                    return Err(crate::cli::Exit(1).into());
                }
                for line in &refused {
                    println!("  {:<10}{line}", paint(BOLD, "refused"));
                }
                anyhow::bail!(match projects.len() {
                    1 => "nothing was started",
                    _ => "nothing was started: every project is checked before any change is made",
                });
            }
            let notes: Vec<(String, String)> = targets
                .iter()
                .flat_map(|t| {
                    let name = t["name"].as_str().unwrap_or("").to_string();
                    t["notes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(move |n| n.as_str().map(|n| (name.clone(), n.to_string())))
                })
                .collect();
            if !json {
                for (name, note) in &notes {
                    match projects.len() {
                        1 => println!("  {}", paint(DIM, note)),
                        _ => println!("  {:<14}{}", name, paint(DIM, note)),
                    }
                }
            }

            let mut v: serde_json::Value = c.post_json("/api/changes", &body).await?;
            // What each run was sent lives on the run: read it back.
            let mut sent_of = Vec::new();
            for started in v["changes"].as_array().cloned().unwrap_or_default() {
                let run_id = started["change"]["runs"]
                    .as_array()
                    .and_then(|r| r.last())
                    .and_then(|r| r.as_str())
                    .map(str::to_string);
                let sent: serde_json::Value = match &run_id {
                    Some(r) => c
                        .get(&format!("/api/runs/{r}"))
                        .await
                        .unwrap_or(serde_json::Value::Null),
                    None => serde_json::Value::Null,
                };
                sent_of.push(sent);
            }
            if json {
                if let Some(o) = v.as_object_mut() {
                    o.insert(
                        "notes".into(),
                        serde_json::json!(notes.iter().map(|(_, n)| n).collect::<Vec<_>>()),
                    );
                    if let [one] = sent_of.as_slice() {
                        o.insert("sent".into(), one["sent"].clone());
                        o.insert("sent_says".into(), one["sent_says"].clone());
                    }
                }
                // A project that changed between preflight and start fails
                // the command here too.
                return crate::cli::print_json(&v);
            }
            let started = v["changes"].as_array().cloned().unwrap_or_default();
            for (one, sent) in started.iter().zip(&sent_of) {
                let w = &one["change"];
                println!(
                    "{} {}",
                    paint(render::GREEN, "started"),
                    paint(BOLD, w["title"].as_str().unwrap_or(&title))
                );
                if projects.len() > 1 {
                    println!("  project   {}", one["project"].as_str().unwrap_or(""));
                }
                if let Some(b) = w["branch"].as_str() {
                    println!("  branch    {b}");
                }
                if let Some(d) = w["worktree"].as_str() {
                    println!("  worktree  {d}");
                }
                if let Some(sp) = w["spec"].as_str() {
                    println!("  spec      {sp}");
                }
                if let Some(says) = sent["sent_says"].as_str() {
                    println!("  tasks     {says}");
                    for t in sent["sent"].as_array().unwrap_or(&vec![]) {
                        println!(
                            "            {}",
                            paint(DIM, t["text"].as_str().unwrap_or(""))
                        );
                    }
                }
                println!(
                    "  {}",
                    paint(
                        DIM,
                        &format!(
                            "devplane change verify {}",
                            one["change_id"].as_str().unwrap_or("")
                        )
                    )
                );
            }
            // A project that changed between preflight and start is named last.
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
        }

        ChangeCmd::Drift {
            change,
            run,
            accept,
            tell,
        } => {
            let c = host?;
            let verb = match (accept, tell) {
                (true, false) => "accept",
                (false, true) => "tell",
                _ => anyhow::bail!(
                    "say which: devplane change drift <id> --run <run> --accept | --tell"
                ),
            };
            let v: serde_json::Value = c
                .post_json(
                    &format!("/api/changes/{change}/drift/{verb}"),
                    &serde_json::json!({ "run": run }),
                )
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            match verb {
                "accept" => println!(
                    "{} the change now works to the specification run {} saw ({}); recorded as yours.",
                    paint(render::GREEN, "accepted —"),
                    clip(&run, 12),
                    clip(v["fingerprint"].as_str().unwrap_or(""), 12)
                ),
                _ => println!(
                    "{} run {} was handed the files that changed; recorded as yours.",
                    paint(render::GREEN, "told —"),
                    clip(&run, 12)
                ),
            }
        }

        ChangeCmd::Retry { change } => {
            let c = host?;
            let v: serde_json::Value = c
                .post_json(
                    &format!("/api/changes/{change}/retry"),
                    &serde_json::json!({}),
                )
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!(
                    "{} the failures went back to the same session.",
                    paint(render::GREEN, "handed back —")
                ),
            }
        }

        ChangeCmd::Resume { change } => {
            let c = host?;
            let v: serde_json::Value = c
                .post_json(
                    &format!("/api/changes/{change}/resume"),
                    &serde_json::json!({}),
                )
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!(
                    "{} the agent is back on the same conversation.",
                    paint(render::GREEN, "resumed —")
                ),
            }
        }

        ChangeCmd::Export { change } => {
            let v: serde_json::Value = crate::local::Reader::open()
                .await?
                .get(&format!("/api/changes/{change}/certificate"))
                .await
                .with_context(|| format!("no change `{change}`"))?;
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            // Unfinished work is not an error: the reply says where it is.
            match json {
                true => println!("{}", serde_json::to_string_pretty(&v["statement"])?),
                false => println!("{}", v["markdown"].as_str().unwrap_or("")),
            }
        }
        ChangeCmd::Show { change } => {
            let all: serde_json::Value = crate::local::Reader::open()
                .await?
                .get("/api/changes")
                .await?;
            let empty = vec![];
            let w = all
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .find(|w| w["id"].as_str() == Some(change.as_str()))
                .with_context(|| format!("no change `{change}`"))?;
            if json {
                println!("{}", serde_json::to_string_pretty(w)?);
                return Ok(());
            }
            print_change(w);
        }
        ChangeCmd::List => {
            let v: serde_json::Value = crate::local::Reader::open()
                .await?
                .get("/api/changes")
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            let empty = vec![];
            let items = v.as_array().unwrap_or(&empty);
            if items.is_empty() {
                println!(
                    "No changes yet.\n\n  {}",
                    paint(BOLD, "devplane change start \"fix the flaky login test\"")
                );
                return Ok(());
            }
            for w in items {
                // State, glyph and gate words are the host's, parsed back into
                // the domain enums so the terminal has no second spelling.
                let state: Option<ChangeState> = serde_json::from_value(w["state"].clone()).ok();
                let standing: Option<Standing> = serde_json::from_value(w["standing"].clone()).ok();
                let says = w["standing_says"].as_str().unwrap_or("");
                let gates = match standing {
                    Some(Standing::Verified) => paint(render::GREEN, says),
                    Some(Standing::Stale { .. } | Standing::ChecksChanged { .. }) => {
                        paint(render::YELLOW, says)
                    }
                    Some(Standing::Failed { .. }) => paint(render::RED, says),
                    _ => paint(DIM, says),
                };
                let waiting = [w["waiting_says"].as_str(), w["in_place_says"].as_str()]
                    .into_iter()
                    .flatten()
                    .map(|s| paint(render::YELLOW, s))
                    .collect::<Vec<_>>()
                    .join("  ");
                let (glyph, word) = match state {
                    Some(s) => (s.glyph(), s.as_str()),
                    None => ("?", "?"),
                };
                println!(
                    "{} {:<40} {:<10} {}{}",
                    match state {
                        Some(ChangeState::Verified) => paint(render::GREEN, glyph),
                        Some(ChangeState::InFlight) => paint(render::BLUE, glyph),
                        Some(ChangeState::Archived) => paint(DIM, glyph),
                        _ => glyph.to_string(),
                    },
                    clip(w["title"].as_str().unwrap_or(""), 40),
                    word,
                    gates,
                    if waiting.is_empty() {
                        String::new()
                    } else {
                        format!("  {waiting}")
                    }
                );
                // The plan beside the verdict: "done, with eleven boxes
                // unticked". The host composes the judgement; this renders it.
                if let Some(plan) = w["plan"].as_object() {
                    let contradicts = w["plan_contradicts_done"].as_bool().unwrap_or(false);
                    let path = plan["path"].as_str().unwrap_or("");
                    let mut parts: Vec<String> = Vec::new();
                    if plan["present"].as_bool() == Some(false) {
                        parts.push(paint(render::RED, "no such specification"));
                    } else if let Some(p) = plan["progress"].as_object() {
                        let done = p["done"].as_u64().unwrap_or(0);
                        let total = p["total"].as_u64().unwrap_or(0);
                        let open = total.saturating_sub(done);
                        parts.push(if open > 0 {
                            paint(render::YELLOW, &format!("{open} of {total} open"))
                        } else {
                            paint(DIM, &format!("all {total} done"))
                        });
                    } else {
                        // Absent is absent. Never `0 of 0`, never a full bar.
                        parts.push(paint(DIM, "no task list"));
                    }
                    let q = plan["open_questions"].as_u64().unwrap_or(0);
                    if q > 0 {
                        parts.push(paint(
                            render::YELLOW,
                            &format!(
                                "{q} question{} nobody answered",
                                if q == 1 { "" } else { "s" }
                            ),
                        ));
                    }
                    println!(
                        "    {} {}",
                        paint(DIM, path),
                        parts.join(paint(DIM, " · ").as_str())
                    );
                    if contradicts {
                        println!(
                            "    {}",
                            paint(render::RED, "done, and the plan it answers is not")
                        );
                    }
                }
                if let Some(shape) = w["shape_says"].as_str() {
                    println!("  {}", paint(DIM, shape));
                }
                let cost = match w["cost_usd"].as_f64().unwrap_or(0.0) {
                    c if c > 0.0 => format!("  ${c:.2}"),
                    _ => String::new(),
                };
                let pr = match w["pull_request"].as_object() {
                    Some(p) => format!(
                        "  #{} {}",
                        p["number"].as_u64().unwrap_or(0),
                        p["status"].as_str().unwrap_or("")
                    ),
                    None => String::new(),
                };
                println!(
                    "  {}",
                    paint(
                        DIM,
                        &format!(
                            "{}  {}{cost}{}",
                            w["id"].as_str().unwrap_or(""),
                            w["branch"].as_str().unwrap_or(""),
                            pr
                        )
                    )
                );
            }
        }

        ChangeCmd::Verify { change } => {
            let c = host?;
            println!("{}", paint(DIM, "running the project's gates…"));
            let v: serde_json::Value = c
                .post_json(
                    &format!("/api/changes/{change}/verify"),
                    &serde_json::json!({}),
                )
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            let passed = v["passed"].as_bool().unwrap_or(false);
            println!(
                "{} {}",
                if passed {
                    paint(render::GREEN, "passed")
                } else {
                    paint(render::RED, "failed")
                },
                v["summary"].as_str().unwrap_or("")
            );
            if !passed {
                let empty = vec![];
                for cmd in v["report"]["commands"].as_array().unwrap_or(&empty) {
                    let empty2 = vec![];
                    for f in cmd["failures"]
                        .as_array()
                        .unwrap_or(&empty2)
                        .iter()
                        .take(10)
                    {
                        println!("    {}", f.as_str().unwrap_or(""));
                    }
                }
            }
        }

        ChangeCmd::Finish { change } => {
            let c = host?;
            let v: serde_json::Value = c
                .post_json(
                    &format!("/api/changes/{change}/finish"),
                    &serde_json::json!({}),
                )
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!(
                    "{} {}",
                    paint(render::GREEN, "finished —"),
                    paint(
                        DIM,
                        &format!(
                            "now {}. `devplane change offer {change}` opens the pull request; \
                             `devplane change archive {change}` removes the worktree.",
                            v["state"].as_str().unwrap_or("recorded")
                        )
                    )
                ),
            }
        }

        ChangeCmd::Adopt {
            branch,
            project,
            spec,
            title,
        } => {
            let c = host?;
            let project = match project {
                Some(p) => p,
                None => {
                    let here = std::env::current_dir()?;
                    crate::repo::find_repo_root(&here).unwrap_or(here)
                }
            };
            let v: serde_json::Value = c
                .post_json(
                    "/api/changes/adopt",
                    &serde_json::json!({
                        "branch": branch,
                        "project": project.to_string_lossy(),
                        "spec": spec,
                        "title": title,
                    }),
                )
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            let w = &v["change"];
            println!(
                "{} {}",
                paint(render::GREEN, "adopted"),
                paint(BOLD, w["title"].as_str().unwrap_or(&branch))
            );
            if let Some(b) = w["branch"].as_str() {
                println!("  branch    {b}");
            }
            if let Some(d) = w["worktree"].as_str() {
                println!("  worktree  {d}");
            }
            if let Some(sp) = w["spec"].as_str() {
                println!("  spec      {sp}");
            }
            println!(
                "  {}",
                paint(
                    DIM,
                    &format!(
                        "devplane change verify {}",
                        v["change_id"].as_str().unwrap_or("")
                    )
                )
            );
        }

        ChangeCmd::Archive {
            change,
            delete_branch,
            discard_uncommitted,
            force,
        } => {
            let c = host?;
            let v: serde_json::Value = c
                .post_json(
                    &format!(
                        "/api/changes/{change}/archive?delete_branch={delete_branch}\
                         &discard_uncommitted={discard_uncommitted}&force={force}"
                    ),
                    &serde_json::json!({}),
                )
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!(
                    "{} {}",
                    paint(render::GREEN, "archived —"),
                    paint(
                        DIM,
                        match (
                            v["worktree_removed"].as_bool().unwrap_or(false),
                            v["branch_deleted"].as_bool().unwrap_or(false),
                        ) {
                            (true, true) => "worktree removed, branch deleted, record kept",
                            (true, false) => "worktree removed, branch and record kept",
                            _ => "nothing to remove; the record is kept",
                        }
                    )
                ),
            }
        }

        ChangeCmd::Review { change, seen, .. } if !seen.is_empty() => {
            let reader = crate::local::Reader::open().await?;
            let mut all = Vec::new();
            for path in &seen {
                let v = reader.mark_seen(&change, path).await?;
                if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                    let rows: Vec<String> = v["rows"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|r| r["path"].as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    anyhow::bail!(
                        "{e}{}",
                        match rows.is_empty() {
                            true => String::new(),
                            false => format!("; the weakened rows are at: {}", rows.join(", ")),
                        }
                    );
                }
                for r in v["seen"].as_array().cloned().unwrap_or_default() {
                    if !json {
                        println!(
                            "seen: {} — {}",
                            r["path"].as_str().unwrap_or(""),
                            r["why"].as_str().unwrap_or("")
                        );
                    }
                    all.push(r);
                }
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({ "seen": all }))?
                );
            }
        }

        ChangeCmd::Review { change, by, .. } => {
            let v: serde_json::Value = crate::local::Reader::open()
                .await?
                .get(&format!("/api/changes/{change}/review"))
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            let r: crate::view::ReviewView =
                serde_json::from_value(v).context("reading the review")?;
            print_review(&r, by == "intent");
        }

        ChangeCmd::Prompt { run, text } => {
            let text = text.join(" ");
            if text.trim().is_empty() {
                anyhow::bail!(
                    "say what to send: devplane change prompt <change|run> \"use the helper\""
                );
            }
            let c = host?;
            // A change id (`c-…`) is addressed through its latest run.
            let run = match run.starts_with("c-") {
                false => run,
                true => {
                    let w: serde_json::Value = c.get(&format!("/api/changes/{run}")).await?;
                    w["runs"]
                        .as_array()
                        .and_then(|r| r.last())
                        .and_then(|r| r.as_str())
                        .map(str::to_string)
                        .with_context(|| format!("change {run} has no run to send to"))?
                }
            };
            let v: serde_json::Value = c
                .post_json(
                    &format!("/api/runs/{run}/prompt"),
                    &serde_json::json!({ "text": text }),
                )
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!("{}", v["says"].as_str().unwrap_or("sent")),
            }
        }

        ChangeCmd::Stop { run, yes } => {
            let c = host?;
            // What survives, said before anything is stopped.
            let before: serde_json::Value = c.get(&format!("/api/runs/{run}/stop")).await?;
            if let Some(e) = before.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            match json {
                true => eprintln!("{}", before["says"].as_str().unwrap_or("")),
                false => println!("{}", before["says"].as_str().unwrap_or("")),
            }
            if !crate::cli::confirm("Stop it?", yes)? {
                eprintln!("{}", paint(DIM, "not stopped"));
                return Ok(());
            }
            let v: serde_json::Value = c
                .post_json(&format!("/api/runs/{run}/stop"), &serde_json::json!({}))
                .await?;
            if json {
                return crate::cli::print_json(&v);
            }
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!("{}", paint(render::GREEN, "stopped")),
            }
        }

        ChangeCmd::Offer { change } => {
            let c = host?;
            let v: serde_json::Value = c
                .post_json(
                    &format!("/api/changes/{change}/offer"),
                    &serde_json::json!({}),
                )
                .await?;
            // A weakened check nobody marked seen: every row, and how to mark one.
            if v["refused"] == "weakened_unseen" {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "refused": v["refused"],
                            "rows": v["rows"],
                            "seen_with": v["seen_with"],
                        }))?
                    );
                    return Err(crate::cli::Exit(REFUSED).into());
                }
                eprintln!(
                    "{} {}\n",
                    paint(BOLD, "refused:"),
                    v["says"].as_str().unwrap_or("")
                );
                for r in v["rows"].as_array().cloned().unwrap_or_default() {
                    eprintln!(
                        "  {}   {}",
                        r["path"].as_str().unwrap_or(""),
                        r["why"].as_str().unwrap_or("")
                    );
                }
                eprintln!(
                    "\nread it in `devplane change review {change}`, then mark it seen:\n  {}",
                    v["seen_with"].as_str().unwrap_or("")
                );
                return Err(crate::cli::Exit(REFUSED).into());
            }
            if json {
                return crate::cli::print_json(&v);
            }
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            match v["offer"].as_str() {
                Some("opened") => println!(
                    "{} #{} {}",
                    paint(render::GREEN, "offered —"),
                    v["pull_request"]["number"].as_u64().unwrap_or(0),
                    paint(DIM, v["pull_request"]["url"].as_str().unwrap_or(""))
                ),
                _ => {
                    // `[github] pull_request` is off: print the command and
                    // the address, run nothing.
                    println!(
                        "{}",
                        paint(
                            DIM,
                            "nothing was pushed — [github] pull_request is not set for this \
                             project: push the branch, then open the address to create the \
                             pull request yourself:"
                        )
                    );
                    println!("  {}", v["push"].as_str().unwrap_or(""));
                    println!("  {}", v["create"].as_str().unwrap_or(""));
                }
            }
        }
    }
    Ok(())
}

/// One change in full: where it is, what it cost, and what its checks said.
fn print_change(w: &serde_json::Value) {
    println!(
        "{}  {}",
        paint(BOLD, w["title"].as_str().unwrap_or("")),
        paint(DIM, w["id"].as_str().unwrap_or(""))
    );
    // Word and glyph come from the enum, parsed back from the host. What the
    // change's own diff weakened follows the word on the same line, never
    // folded into it.
    let state: Option<ChangeState> = serde_json::from_value(w["state"].clone()).ok();
    let qualified = match w["qualifier"]["says"].as_str().filter(|s| !s.is_empty()) {
        Some(q) => format!(
            " · {}",
            paint(
                render::YELLOW,
                &format!(
                    "{q} ({} unseen)",
                    w["qualifier"]["unseen"].as_u64().unwrap_or(0)
                )
            )
        ),
        None => String::new(),
    };
    println!(
        "  state      {} {}{qualified}{}{}",
        state.map_or("?", ChangeState::glyph),
        state.map_or("?", ChangeState::as_str),
        match w["waiting_says"].as_str() {
            Some(s) => format!(" — {}", paint(render::YELLOW, s)),
            None => String::new(),
        },
        match w["in_place_says"].as_str() {
            Some(s) => format!(" — {}", paint(render::YELLOW, s)),
            None => String::new(),
        }
    );
    if let Some(at) = w["archived_at"].as_str() {
        println!("  archived   {at}");
    }
    if let Some(shape) = w["shape_says"].as_str() {
        println!("  shape      {shape}");
    }
    if let Some(says) = w["standing_says"].as_str() {
        let standing: Option<Standing> = serde_json::from_value(w["standing"].clone()).ok();
        let colour = match standing {
            Some(Standing::Verified) => render::GREEN,
            Some(Standing::Failed { .. }) => render::RED,
            Some(Standing::Stale { .. } | Standing::ChecksChanged { .. }) => render::YELLOW,
            _ => DIM,
        };
        println!("  gates      {}", paint(colour, says));
    }
    if let Some(b) = w["branch"].as_str() {
        println!("  branch     {b}");
    }
    if let Some(d) = w["worktree"].as_str() {
        println!("  worktree   {d}");
    }
    // The spec and its fingerprint when the gate last ran; the path alone
    // stops meaning anything once the file moves.
    if let Some(sp) = w["spec"].as_str() {
        let stamp = &w["gate"]["spec"];
        match (stamp["path"].as_str(), stamp["fingerprint"].as_str()) {
            (Some(_), Some(fp)) => {
                let files = stamp["files"].as_u64().unwrap_or(1);
                let over = match files > 1 {
                    true => format!(" over {files} documents"),
                    false => String::new(),
                };
                println!(
                    "  spec       {sp} {}",
                    paint(DIM, &format!("at {fp}{over}"))
                )
            }
            (Some(_), None) => println!(
                "  spec       {sp} {}",
                paint(render::RED, "— not there when the gate ran")
            ),
            _ => println!(
                "  spec       {sp} {}",
                paint(DIM, "— no gate has run against it yet")
            ),
        }
        // The spec's task list as of the gate run, counted, never judged.
        let total = stamp["tasks_total"].as_u64().unwrap_or(0);
        if total > 0 {
            let done = stamp["tasks_done"].as_u64().unwrap_or(0);
            let left = total - done;
            println!(
                "             {} {}",
                paint(BOLD, &format!("{done}/{total} tasks")),
                match left {
                    0 => paint(DIM, "every box ticked"),
                    _ => paint(
                        render::YELLOW,
                        &format!("{left} still open in the specification")
                    ),
                }
            );
        }
        let questions = stamp["open_questions"].as_u64().unwrap_or(0);
        if questions > 0 {
            println!(
                "             {}",
                paint(
                    render::YELLOW,
                    &format!(
                        "{questions} unanswered question{} in the specification",
                        if questions == 1 { "" } else { "s" }
                    )
                )
            );
        }
    }
    // Ticked (what the agent wrote) and verified (a gate exit over a run that
    // was sent the task) are never merged. Ticked-but-unsent is listed.
    if let Some(says) = w["counts_says"].as_str() {
        println!("  tasks      {}", paint(BOLD, says));
        let unsent: Vec<&str> = w["counts"]["ticked_unsent"]
            .as_array()
            .map(|a| a.iter().filter_map(|t| t.as_str()).collect())
            .unwrap_or_default();
        if !unsent.is_empty() {
            println!(
                "             {}",
                paint(
                    render::YELLOW,
                    &format!("ticked, sent to nobody: {}", unsent.join(", "))
                )
            );
        }
        for row in w["token_rows"].as_array().unwrap_or(&vec![]) {
            println!(
                "  {:<10} {}",
                row["token"].as_str().unwrap_or("?"),
                row["says"].as_str().unwrap_or("")
            );
        }
    }
    for d in w["drifts"].as_array().unwrap_or(&vec![]) {
        println!(
            "  drift      {}",
            paint(render::YELLOW, d["says"].as_str().unwrap_or(""))
        );
        println!(
            "             {}",
            paint(
                DIM,
                &format!(
                    "devplane change drift {} --run {} --accept | --tell",
                    w["id"].as_str().unwrap_or(""),
                    d["run"].as_str().unwrap_or("")
                )
            )
        );
    }
    let empty = vec![];
    let runs = w["runs"].as_array().unwrap_or(&empty);
    if !runs.is_empty() {
        println!(
            "  runs       {}",
            runs.iter()
                .filter_map(|r| r.as_str())
                .map(|r| clip(r, 12))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if w["feedback_rounds"].as_u64().unwrap_or(0) > 0 {
        println!(
            "  handed back {} time(s)",
            w["feedback_rounds"].as_u64().unwrap_or(0)
        );
    }
    // A zero cost usually means the agent does not report one, so a `[budget]`
    // ceiling cannot bite; `$0.00` would hide that.
    match w["cost_usd"].as_f64().unwrap_or(0.0) {
        c if c > 0.0 => println!("  cost       ${c:.2}"),
        _ => println!(
            "  cost       {}",
            paint(
                DIM,
                "not reported by this agent — a [budget] ceiling cannot bite"
            )
        ),
    }
    // Why it stopped, from the row: several reasons reach `failed`.
    if let Some(stopped) = w["stopped"].as_object() {
        let line = match stopped.get("reason").and_then(|r| r.as_str()) {
            Some("over_budget") => format!(
                "stopped at ${:.2}, over the ceiling [budget] sets",
                stopped["spent_usd"].as_f64().unwrap_or_default()
            ),
            Some("gate_failed") => format!(
                "{} failed and the feedback budget is spent",
                stopped["gate"].as_str().unwrap_or("the gate")
            ),
            Some("review_exhausted") => format!(
                "{} kept finding things and sent the change back to {} as often as allowed",
                stopped["step"].as_str().unwrap_or("the reviewer"),
                stopped["back_to"].as_str().unwrap_or("an earlier step")
            ),
            Some("broken") => format!(
                "the chain could not continue: {}",
                stopped["detail"].as_str().unwrap_or("no detail recorded")
            ),
            _ => "stopped for a reason this build does not recognise".to_string(),
        };
        println!("  {}", paint(render::YELLOW, &line));
        if let Some(f) = stopped.get("findings").and_then(|f| f.as_str()) {
            for l in f.lines().take(10) {
                println!("    {}", paint(DIM, l));
            }
        }
    }
    if let Some(pr) = w["pull_request"].as_object() {
        println!(
            "  pull req   #{} {} {}",
            pr["number"].as_u64().unwrap_or(0),
            pr["status"].as_str().unwrap_or(""),
            paint(DIM, pr["url"].as_str().unwrap_or(""))
        );
    }

    if let Some(setup) = w["setup"].as_object() {
        let ok = setup["commands"]
            .as_array()
            .map(|c| c.iter().all(|r| r["outcome"]["code"] == 0))
            .unwrap_or(false);
        println!(
            "\n{} {}",
            paint(BOLD, "setup"),
            if ok {
                paint(render::GREEN, "ok")
            } else {
                paint(render::RED, "failed")
            }
        );
    }

    let gates = w["gates"].as_array().unwrap_or(&empty);
    for g in gates.iter().rev().take(3) {
        let passed = w["gate"]["passed"].as_bool().unwrap_or(false)
            && std::ptr::eq(g, gates.last().unwrap());
        println!(
            "\n{} {} {}",
            paint(BOLD, g["gate"].as_str().unwrap_or("gate")),
            paint(
                DIM,
                &format!(
                    "attempt {} of {}",
                    g["attempt"].as_u64().unwrap_or(1),
                    gates.len()
                )
            ),
            if passed {
                paint(render::GREEN, "passed")
            } else {
                String::new()
            }
        );
        for cmd in g["commands"].as_array().unwrap_or(&empty) {
            // Exited non-zero, timed out, never started and undeterminable are
            // different; only the first is a verdict about the change.
            let outcome = &cmd["outcome"];
            let kind = outcome["outcome"].as_str().unwrap_or("");
            let (glyph, colour, note) = match kind {
                "exited" if outcome["code"] == 0 => ("✓", render::GREEN, String::new()),
                "exited" => (
                    "✗",
                    render::RED,
                    format!("exit {}", outcome["code"].as_i64().unwrap_or(-1)),
                ),
                "timed_out" => (
                    "⏱",
                    render::RED,
                    format!(
                        "timed out after {}s",
                        outcome["after_secs"].as_u64().unwrap_or(0)
                    ),
                ),
                "never_started" => (
                    "–",
                    DIM,
                    format!(
                        "never started: {}",
                        outcome["reason"].as_str().unwrap_or("")
                    ),
                ),
                _ => (
                    "?",
                    DIM,
                    format!(
                        "could not be determined: {}",
                        outcome["reason"].as_str().unwrap_or("")
                    ),
                ),
            };
            println!(
                "  {} {} {}",
                paint(colour, glyph),
                cmd["command"].as_str().unwrap_or(""),
                paint(DIM, &note)
            );
            for f in cmd["failures"].as_array().unwrap_or(&empty).iter().take(8) {
                println!("      {}", paint(DIM, f.as_str().unwrap_or("")));
            }
        }
    }

    // The agent's own account, printed only beside the measured lines and
    // judged by neither: the reader decides.
    if let Some(claim) = w["claim"].as_str() {
        println!("\n{}", paint(BOLD, "the agent's account"));
        for line in crate::core::text::wrap(claim, 68) {
            println!("  {}", paint(DIM, &line));
        }
        println!(
            "  {}",
            paint(DIM, "— read beside the lines above, which are measured")
        );
    }
}

/// The issues one repository offers as work: `devplane forge issues --ready`.
/// The cross-project list lives in `cli::board`.
pub async fn cmd_ready_issues(
    cwd: Option<std::path::PathBuf>,
    label: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let c = client::Client::connect_running().await?;
    let cwd = match cwd {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    let v: serde_json::Value = c
        .post_json(
            "/api/issues",
            &serde_json::json!({ "cwd": cwd.to_string_lossy(), "label": label }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        anyhow::bail!("{e}");
    }
    let empty = vec![];
    let issues = v.as_array().unwrap_or(&empty);
    if issues.is_empty() {
        println!("{}", paint(DIM, "no issues are on offer here"));
        return Ok(());
    }
    for i in issues {
        println!(
            "{:>6}  {}",
            paint(BOLD, &format!("#{}", i["number"].as_u64().unwrap_or(0))),
            clip(i["title"].as_str().unwrap_or(""), 70)
        );
    }
    println!(
        "\n  {}",
        paint(DIM, "devplane change start --issue <number>")
    );
    Ok(())
}

/// Allow rules from the agent's own settings: `devplane.toml` has no allow
/// list, so these are the rules that actually grant.
fn vendor_allow_rules(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    for rel in [".claude/settings.json", ".claude/settings.local.json"] {
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if let Some(a) = v
            .get("permissions")
            .and_then(|p| p.get("allow"))
            .and_then(|a| a.as_array())
        {
            out.extend(a.iter().filter_map(|r| r.as_str().map(str::to_string)));
        }
    }
    out
}

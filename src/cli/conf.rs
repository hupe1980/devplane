//! `devplane config`: reading a repository's `devplane.toml` back, and asking
//! what its rules would decide about one call.

use crate::render;
use crate::render::{BOLD, DIM, YELLOW, paint};
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

/// `devplane config explain` — what the gate would decide about one call, and why.
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
                "every call here is asked until it parses — devplane config check"
            )
        );
    }
    if !problems.is_empty() {
        println!(
            "\n{}",
            paint(
                YELLOW,
                &format!(
                    "{} rule(s) in this project cannot do what they say — devplane config check",
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
                    "no tool calls observed in {named} — `devplane agent connect claude`, then come back"
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
    let problems = config.validate();

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
            true => "yes, as a draft",
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
                paint(
                    DIM,
                    &format!("add {add} to [policy] deny to stop writes too")
                )
            );
        }
    }
    if !problems.is_empty() {
        println!();
    }
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

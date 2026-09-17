//! Work: starting it, checking a project's configuration, and reading a verdict.

use super::WorkCmd;
use crate::render::{BOLD, DIM, YELLOW, clip, paint};
use crate::{client, render};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub async fn cmd_dispatch(
    agent: &str,
    cwd: Option<PathBuf>,
    prompt: String,
    json: bool,
) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let cwd = match cwd {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    let body = serde_json::json!({
        "agent": agent,
        "cwd": cwd.to_string_lossy(),
        "prompt": (!prompt.is_empty()).then_some(prompt),
    });
    let v: serde_json::Value = c.post_json("/api/dispatch", &body).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    match v.get("error").and_then(|e| e.as_str()) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!(
            "{} {} in {}\n  {}",
            paint(render::GREEN, "started"),
            v["agent"].as_str().unwrap_or(agent),
            cwd.display(),
            paint(
                DIM,
                &format!("vibeplane say {} ...", v["run_id"].as_str().unwrap_or(""))
            )
        ),
    }
    Ok(())
}

/// Where the rules governing a directory came from, and whether they loaded.
///
/// `undecided` was one sentence covering three situations and only one of them
/// is a fact about the call. Telling them apart is one rule, on the surface
/// where it matters most: **a typo in a deny rule must never read as
/// permission**, and "no rule answers this one" is exactly how it read.
enum Rules {
    /// No `vibeplane.toml` governs this directory at all.
    None { root: PathBuf },
    /// One was found and will not load, so *nothing* is in force here.
    Broken { path: PathBuf, error: String },
    /// Loaded. `problems` are the rules that cannot do what they say.
    Loaded {
        path: PathBuf,
        count: usize,
        problems: Vec<crate::core::config::Problem>,
        /// Rules that provably do nothing — see `Policy::redundancies`. Carried
        /// here so `--json` says it too: a finding only the terminal prints is
        /// one no script can act on.
        unused: Vec<String>,
        /// Allow rules that grant an arbitrary program — see
        /// `Policy::overbroad`. Here for the same reason `unused` is.
        overbroad: Vec<crate::core::policy::Overbroad>,
    },
}

impl Rules {
    fn at(dir: &Path) -> Self {
        let root = crate::core::project::governing_root(dir).unwrap_or_else(|| dir.to_path_buf());
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
                    count: p.allow_rules().len() + p.deny_rules().len() + p.ask_rules().len(),
                    problems: c.validate(),
                    unused: p.redundancies(),
                    overbroad: p.overbroad(),
                    path,
                }
            }
        }
    }

    fn problems(&self) -> Vec<crate::core::config::Problem> {
        match self {
            Rules::Loaded { problems, .. } => problems.clone(),
            // A file that will not load has no rules to find problems in, and
            // saying "0 problems" about it would be the same lie one level
            // down. `load_error` is what that case reports.
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
                "no vibeplane.toml governs this directory, so nothing here decides anything"
            }
            Rules::Broken { .. } => {
                "this project's rules are NOT in force — the file below will not load"
            }
            Rules::Loaded { count: 0, .. } => {
                "this project's vibeplane.toml declares no [policy] rules"
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
                // Spelled out so a script cannot read this as "nothing matched".
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

/// `vibeplane explain` — what the gate would decide about one call, and why.
///
/// Offline and daemon-free, like `check`, and for the same two reasons. A
/// person editing `[policy]` needs to be able to ask a question of the rules
/// they just wrote without starting an agent to find out; and the answer has to
/// be scriptable, because the only thing that can keep this layer honest is
/// asking both sides about calls nobody wrote down
/// (`scripts/verify-permissions-diff.sh`).
///
/// The interesting output is not the verdict — it is the `undecided` that
/// **looks like** an allow. A rule can match and still not answer: the command
/// writes somewhere no rule covers, or it is a form Claude Code puts in front
/// of a person whatever the rules say. Those used to be invisible, and they are
/// exactly where this layer has been wrong six times.
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

    // The gate this machine actually enforces, not a project-only imitation of
    // it: `explain` used `for_projects_only()` and so answered without
    // `~/.vibeplane/policy.toml`, which let the surface built to say *what
    // would the gate decide* answer `allow` for a call the machine denies.
    let (cache, global_error) = crate::core::PolicyCache::from_disk();
    let verdict = cache.evaluate(&dir, &tool, &input);
    let restrictive = cache.restrictive(&dir, &tool, &input);

    // **Where the rules came from, and whether they loaded.** `undecided` used
    // to be one sentence covering three different situations, and only one of
    // them is a fact about the call: no rules here, rules that would not load,
    // and rules that loaded and did not match. The middle one is the dangerous
    // one — a `vibeplane.toml` with a typo in it has *no* rules in force, and
    // reporting that as "no rule answers this one" is a typo in a deny rule
    // reading as permission, which is the one thing this layer may never do.
    //
    // `vibeplane check` has always printed the parse error. This is the surface
    // somebody uses *while editing the file*, so it is the surface most likely
    // to meet a broken one.
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
                // What a `PreToolUse` hook would answer, which is a different
                // question and the one that holds in auto mode.
                "restrictive": restrictive.as_str(),
                // Which of the three `undecided` situations this is.
                "rules": rules.as_json(),
            }))?
        );
        return Ok(());
    }

    let (colour, headline) = match &verdict {
        crate::core::Verdict::Allow { .. } => (render::GREEN, "allow"),
        crate::core::Verdict::Deny { .. } => (render::RED, "deny"),
        crate::core::Verdict::Ask { .. } => (YELLOW, "ask"),
        crate::core::Verdict::Undecided => (DIM, "undecided"),
    };
    println!("{}  {}", paint(colour, headline), paint(BOLD, &tool));
    match verdict.rule() {
        Some(r) => println!("        {}", paint(DIM, &format!("by {r}"))),
        None => println!("        {}", paint(DIM, rules.why_nothing_answered())),
    }
    if let Some(e) = &global_error {
        println!(
            "\n{}\n{}",
            paint(render::RED, "the machine-wide rules are NOT in force"),
            paint(DIM, e)
        );
    }
    if let Some(broken) = rules.load_error() {
        println!("\n{}", paint(render::RED, &broken));
        println!(
            "{}",
            paint(
                DIM,
                "every rule in this file is off until it parses — vibeplane check"
            )
        );
    }
    if !problems.is_empty() {
        println!(
            "\n{}",
            paint(
                YELLOW,
                &format!(
                    "{} rule(s) in this project cannot do what they say — vibeplane check",
                    problems.len()
                )
            )
        );
    }
    Ok(())
}

/// Replay every tool call this machine has seen against the rules as they are
/// now, and say which rule would stop the interruptions.
///
/// The product measures its own inbox (`vibeplane attention`); this is the
/// other half — what to *do* about it. A permission prompt that has appeared
/// forty times is forty interruptions a person could have answered once, and
/// the thing standing between them and answering it once is knowing that the
/// `*` goes after the subcommand and that a rule which cannot match anything
/// reads as protection and is none.
///
/// Offline on purpose, like the rest of `explain`: it opens the store
/// read-only and asks no agent anything, so it costs nothing and can be run
/// while a rule is still being written.
pub async fn cmd_replay(dir: PathBuf, limit: i64, json: bool) -> Result<()> {
    let dir = dir.canonicalize().context("that path does not exist")?;
    let store = crate::store::Store::open(&crate::config::db_path()?).await?;
    // `--dir` scopes the replay to the calls made inside it, so
    // `--replay --dir ~/work/saas` asks "what rules should *this* project
    // have". The default is the working directory, and an answer about every
    // project at once would be a list nobody can paste anywhere — so the
    // scope is the repository root when there is one, not the exact directory,
    // because a session started in `src/` belongs to the same project.
    let scope = crate::core::project::find_repo_root(&dir).unwrap_or_else(|| dir.clone());
    // The name rather than the path: a board groups by project name and this
    // is the same question, and an absolute path in a headline is noise.
    let named = scope
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| scope.display().to_string());
    let calls = store.observed_tool_calls(Some(&scope), limit).await?;

    let cache = crate::core::PolicyCache::for_projects_only();
    let mut counts = [0usize; 4]; // allow, ask, deny, undecided
    // Commands that reached a person, grouped by the rule that would answer
    // them. `BTreeMap` so two runs of this print the same thing.
    let mut open: std::collections::BTreeMap<String, Vec<String>> = Default::default();

    for c in &calls {
        let verdict = cache.evaluate(&c.cwd, &c.tool, &c.input);
        let slot = match verdict {
            crate::core::Verdict::Allow { .. } => 0,
            crate::core::Verdict::Ask { .. } => 1,
            crate::core::Verdict::Deny { .. } => 2,
            crate::core::Verdict::Undecided => 3,
        };
        counts[slot] += 1;
        if slot != 3 {
            continue;
        }
        let Some(text) = crate::core::policy::rule_content(&c.tool, &c.input) else {
            continue;
        };
        // Grouped by the *family* a rule would name — the program and its
        // subcommand — rather than by tool. One bucket per tool asks
        // `suggest_rule_for` to find a single rule covering `pnpm test` and
        // `rm -rf node_modules`, which it rightly refuses, and the answer is
        // then "nothing worth writing" for a screen full of interruptions.
        let family = crate::core::command::rule_family(&c.tool, &text);
        open.entry(format!("{}\u{0}{}", c.tool, family))
            .or_default()
            .push(text);
    }

    // One suggestion per family, and only where a rule really would cover it.
    //
    // **A rule is worth writing when the prompt keeps coming back.** Below
    // that, a standing grant costs more than the interruption it saves — and
    // suggesting one for a call somebody made once is how a list like this
    // ends up recommending `rm -rf node_modules`. Three is the smallest number
    // that means "again".
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
                "allow": counts[0], "ask": counts[1],
                "deny": counts[2], "undecided": counts[3],
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
                    "no tool calls observed in {named} — `vibeplane connect claude`, then come back"
                )
            )
        );
        return Ok(());
    }

    let pct = |n: usize| (n as f64 * 100.0 / total as f64).round() as u64;
    println!("{total} tool calls in {named} replayed against the rules as they are now\n");
    for (n, label, colour) in [
        (counts[0], "allow", render::GREEN),
        (counts[1], "ask", YELLOW),
        (counts[2], "deny", render::RED),
        (counts[3], "reached you", DIM),
    ] {
        if n > 0 {
            println!("  {:>6}  {:>3}%  {}", n, pct(n), paint(colour, label));
        }
    }

    if advice.is_empty() {
        println!(
            "\n{}",
            paint(DIM, "nothing that reached you has a rule worth writing")
        );
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
                "{shown} of the {} calls that reached you would stop asking · \
                 paste into [policy] auto_allow, then vibeplane check",
                counts[3]
            )
        )
    );
    let _ = answerable;
    Ok(())
}

pub fn cmd_check(path: PathBuf, json: bool) -> Result<()> {
    let root = path.canonicalize().context("that path does not exist")?;
    let root = crate::core::project::find_repo_root(&root).unwrap_or(root);
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
                std::process::exit(1);
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
                // The same read-back the board shows, from the same function:
                // a terminal and a browser disagreeing about a repository's own
                // rules is the failure one derivation exists to prevent. It
                // carries `problems` itself, so they are not repeated here.
                "describes": config.describe(),
            }))?
        );
        if problems.iter().any(|p| p.fatal) {
            std::process::exit(1);
        }
        return Ok(());
    }

    if !file.exists() {
        println!(
            "{}\n\nNothing is verified and nothing pretends to be. \
             Add a vibeplane.toml when you want gates.",
            paint(DIM, &format!("no {}", file.display()))
        );
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

    // What it will actually do, which is the other half of the question.
    println!("{}", paint(render::GREEN, "ok"));
    println!(
        "  gates     {}",
        match config.gates.check.is_empty() {
            true => paint(DIM, "none — nothing is verified"),
            false => config.gates.check.join(" && "),
        }
    );
    for (name, gate) in &config.gates.named {
        println!(
            "            {name}: {} {}",
            gate.run.join(" && "),
            paint(
                DIM,
                match gate.expect {
                    crate::core::config::Expect::Fail => "(must fail)",
                    crate::core::config::Expect::Pass => "",
                }
            )
        );
    }
    for (name, pipeline) in &config.pipelines {
        println!(
            "  {name:<9} {}",
            pipeline
                .steps
                .iter()
                .map(|s| match s {
                    crate::core::config::Step::Human(h) => format!("⏸{}", h.human),
                    crate::core::config::Step::Role(r) => r.role.clone(),
                })
                .collect::<Vec<_>>()
                .join(" › ")
        );
    }
    println!(
        "  pull req  {}",
        match config.github.pull_request {
            true if config.github.draft => "yes, as a draft",
            true => "yes",
            false => "no",
        }
    );
    // What the rules actually do, which is the half of "is this file right"
    // that the problem list cannot answer. A rule that parses, is legal and
    // still covers nothing anybody expected is only visible by reading it back.
    let policy = config.policy();
    if !policy.is_empty() {
        println!(
            "  policy    {} deny, {} ask, {} allow",
            policy.deny_rules().len(),
            policy.ask_rules().len(),
            policy.allow_rules().len()
        );
        // Printed in the order they are evaluated, because that order is the
        // thing most likely to surprise: a matching ask beats a narrower allow.
        // A negation carves a hole in the list it sits in, so it is labelled
        // `except` rather than by the list's own name: `deny  !Bash(git status)`
        // reads at a glance as "deny this", which is the opposite of what it
        // does, and a rule whose badge inverts its meaning is worse than one
        // nobody printed.
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
        for rule in policy.allow_rules() {
            println!("            {} {}", paint(render::GREEN, "allow "), rule);
        }
        // Advice, printed once and not per rule. A `Read` deny does exactly
        // what it says; what it does not say — that a shell redirection and
        // `touch` are `Edit` business — is the part people get wrong, and
        // `Read(.env)` is the most common rule anybody writes. As a per-rule
        // warning this would fire on the canonical example and teach people to
        // ignore warnings.
        // A rule that provably does nothing. Unlike the note below it, this is
        // a *finding* rather than a suggestion: it is answered by pattern
        // containment rather than by a heuristic, and it under-reports on
        // purpose, so anything it prints is worth a person's attention.
        let dead = policy.redundancies();
        if !dead.is_empty() {
            println!();
        }
        for finding in &dead {
            let mut lines = crate::core::text::wrap(finding, 66).into_iter();
            if let Some(first) = lines.next() {
                println!("  {:<10}{}", paint(BOLD, "unused"), first);
            }
            for rest in lines {
                println!("  {:<10}{}", "", rest);
            }
        }
        // A rule that provably grants more than it reads as granting. A
        // *finding* like `unused` and for the same reason — pattern
        // containment, not a heuristic — and printed above the advice below
        // because it is the only line here that is about somebody getting more
        // than they asked for.
        let wide = policy.overbroad();
        if !wide.is_empty() {
            println!();
        }
        for finding in &wide {
            println!("  {:<10}{}", paint(BOLD, "overbroad"), finding.rule);
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
                "\n  {:<10}those denies stop reads only — `echo x > {first}` and `touch {first}` still run",
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

/// `vibeplane trust` — show what a repository's agent configuration does, then
/// let a person decide.
///
/// The scan is the point. `trust` has always been the one deliberate act in
/// this product, and for as long as it has existed it asked a person to make
/// that decision with nothing in front of them. A gate whose evidence is the
/// word "trusted" is a consent dialog.
///
/// Three rules, and the first is the one that keeps the other two worth having.
/// **It reports and refuses nothing** — plenty of repositories legitimately
/// ship a `PreToolUse` hook, and a tool that graded them would teach people to
/// skip the one prompt here that matters. **A repository with none of this
/// prints nothing extra**, so the common case is not made noisy by a feature
/// aimed at the uncommon one. And **`--dry-run` trusts nothing**, which is the
/// form to run on somebody else's repository.
pub async fn cmd_trust(path: PathBuf, yes: bool, dry_run: bool, json: bool) -> Result<()> {
    let root = path.canonicalize().context("that path does not exist")?;
    let setup = crate::core::setup::scan(&root);

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
    // A repository with nothing in it is not worth a second keystroke: the
    // prompt exists to make evidence unskippable, and there is no evidence.
    if !yes && !setup.is_empty() && !confirm("Trust this repository?")? {
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
    let c = client::Client::connect_or_start().await?;
    c.post_json(
        "/api/projects/trust",
        &serde_json::json!({ "path": root.to_string_lossy() }),
    )
    .await
}

/// What starting an agent here will load, grouped the way a person reads it.
fn print_setup(root: &Path, setup: &crate::core::setup::Setup) {
    if setup.is_empty() {
        println!(
            "  {}",
            paint(
                DIM,
                &format!(
                    "{} declares no hooks, MCP servers or skills",
                    root.display()
                )
            )
        );
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

/// A yes/no question on the terminal. Default no.
///
/// Not a prompt library: one question, one line, and a non-interactive stdin
/// answers *no* rather than hanging — a script that pipes nothing into `trust`
/// should fail closed and be told to pass `--yes`.
fn confirm(question: &str) -> Result<bool> {
    use std::io::Write;
    print!("  {question} [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer)? == 0 {
        println!();
        anyhow::bail!("nothing on stdin to answer with — pass --yes to trust without asking");
    }
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

pub async fn cmd_work(what: WorkCmd, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    match what {
        WorkCmd::Start {
            title,
            kind,
            agent,
            cwd,
            no_worktree,
            issue,
            spec,
        } => {
            let cwd = match cwd {
                Some(p) => p,
                None => std::env::current_dir()?,
            };
            let title = title.join(" ");
            if title.is_empty() && issue.is_none() {
                anyhow::bail!(
                    "say what the work is: vibeplane work start \"fix the flaky test\"\n\
                     or point at an issue:  vibeplane work start --issue 7"
                );
            }
            let v: serde_json::Value = c
                .post_json(
                    "/api/work",
                    &serde_json::json!({
                        "cwd": cwd.to_string_lossy(),
                        "title": title,
                        "kind": kind,
                        "agent": agent,
                        "worktree": !no_worktree,
                        "issue": issue,
                        "spec": spec,
                    }),
                )
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            let w = &v["work"];
            println!(
                "{} {}",
                paint(render::GREEN, "started"),
                paint(BOLD, w["title"].as_str().unwrap_or(&title))
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
                        "vibeplane work verify {}",
                        v["work_id"].as_str().unwrap_or("")
                    )
                )
            );
        }

        WorkCmd::Approve { work } => {
            let v: serde_json::Value = c
                .post_json(&format!("/api/work/{work}/approve"), &serde_json::json!({}))
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            match v["released"].as_str() {
                Some(step) => println!("{} released; the pipeline continues.", paint(BOLD, step)),
                None => println!("released."),
            }
        }

        WorkCmd::Retry { work } => {
            let v: serde_json::Value = c
                .post_json(&format!("/api/work/{work}/retry"), &serde_json::json!({}))
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!(
                    "{} the failures went back to the same session.",
                    paint(render::GREEN, "handed back —")
                ),
            }
        }

        WorkCmd::Resume { work } => {
            let v: serde_json::Value = c
                .post_json(&format!("/api/work/{work}/resume"), &serde_json::json!({}))
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!(
                    "{} the agent is back on the same conversation.",
                    paint(render::GREEN, "resumed —")
                ),
            }
        }

        WorkCmd::Show { work } => {
            let all: serde_json::Value = c.get("/api/work").await?;
            let empty = vec![];
            let w = all
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .find(|w| w["id"].as_str() == Some(work.as_str()))
                .with_context(|| format!("no work `{work}`"))?;
            if json {
                println!("{}", serde_json::to_string_pretty(w)?);
                return Ok(());
            }
            print_work(w);
        }
        WorkCmd::List => {
            let v: serde_json::Value = c.get("/api/work").await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            let empty = vec![];
            let items = v.as_array().unwrap_or(&empty);
            if items.is_empty() {
                println!(
                    "No work yet.\n\n  {}",
                    paint(BOLD, "vibeplane work start \"fix the flaky login test\"")
                );
                return Ok(());
            }
            for w in items {
                let phase = w["phase"].as_str().unwrap_or("?");
                // The verdict is the daemon's, not re-derived here: a
                // reproduction gate passes *by failing*, and a second copy of
                // that rule in the terminal got it backwards.
                let gate = match w["gate"]["passed"].as_bool() {
                    Some(true) => paint(render::GREEN, "gates green"),
                    Some(false) => paint(render::RED, "gates red"),
                    None => String::new(),
                };
                println!(
                    "{} {:<40} {:<10} {}",
                    match phase {
                        "review" => paint(render::GREEN, "✓"),
                        "failed" => paint(render::RED, "✗"),
                        "verify" => paint(render::YELLOW, "◆"),
                        "human" => paint(render::YELLOW, "⏸"),
                        "done" => paint(DIM, "·"),
                        _ => paint(render::BLUE, "●"),
                    },
                    clip(w["title"].as_str().unwrap_or(""), 40),
                    phase,
                    gate
                );
                // The stepper is the whole story of a pipeline in one line:
                // what it has done, where it is, what is left.
                if let Some(p) = w["pipeline"].as_object() {
                    let roles: Vec<&str> = p["roles"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|r| r.as_str()).collect())
                        .unwrap_or_default();
                    let at = p["step"].as_u64().unwrap_or(0) as usize;
                    let line = roles
                        .iter()
                        .enumerate()
                        .map(|(i, r)| match i.cmp(&at) {
                            std::cmp::Ordering::Less => paint(render::GREEN, &format!("✓{r}")),
                            std::cmp::Ordering::Equal => paint(BOLD, &format!("▸{r}")),
                            std::cmp::Ordering::Greater => paint(DIM, r),
                        })
                        .collect::<Vec<_>>()
                        .join(paint(DIM, " › ").as_str());
                    println!("  {line}");
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

        WorkCmd::Verify { work } => {
            println!("{}", paint(DIM, "running the project's gates…"));
            let v: serde_json::Value = c
                .post_json(&format!("/api/work/{work}/verify"), &serde_json::json!({}))
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
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

        WorkCmd::Finish {
            work,
            remove_worktree,
            force,
        } => {
            let v: serde_json::Value = c
                .post_json(
                    &format!(
                        "/api/work/{work}/finish?remove_worktree={remove_worktree}&force={force}"
                    ),
                    &serde_json::json!({}),
                )
                .await?;
            match v.get("error").and_then(|e| e.as_str()) {
                Some(e) => anyhow::bail!("{e}"),
                None => println!("{}", paint(render::GREEN, "done")),
            }
        }
    }
    Ok(())
}

/// One piece of work in full: where it is, what it cost, and what its checks
/// said. `work list` answers "what is going on"; this answers "why is this one
/// red", without reading the daemon log.
fn print_work(w: &serde_json::Value) {
    let phase = w["phase"].as_str().unwrap_or("?");
    println!(
        "{}  {}",
        paint(BOLD, w["title"].as_str().unwrap_or("")),
        paint(DIM, w["id"].as_str().unwrap_or(""))
    );
    println!("  phase      {phase}");
    println!("  kind       {}", w["kind"].as_str().unwrap_or("?"));
    if let Some(b) = w["branch"].as_str() {
        println!("  branch     {b}");
    }
    if let Some(d) = w["worktree"].as_str() {
        println!("  worktree   {d}");
    }
    // What this work answers, and what that specification *was* when the gate
    // last ran. The path alone stops being a claim a reviewer can act on the
    // moment the file moves.
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
        // What the specification's own task list said when the gate ran.
        //
        // Printed under the gate rather than beside the path, because the
        // number is only worth anything next to a verdict: *the gate passed and
        // eleven boxes are unticked* is a sentence neither the exit code nor
        // the agent's own account can produce alone. Counted, never judged.
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
    if let Some(p) = w["pipeline"].as_object() {
        let roles: Vec<&str> = p["roles"]
            .as_array()
            .map(|a| a.iter().filter_map(|r| r.as_str()).collect())
            .unwrap_or_default();
        let at = p["step"].as_u64().unwrap_or(0) as usize;
        println!(
            "  pipeline   {}",
            roles
                .iter()
                .enumerate()
                .map(|(i, r)| match i.cmp(&at) {
                    std::cmp::Ordering::Less => paint(render::GREEN, &format!("✓{r}")),
                    std::cmp::Ordering::Equal => paint(BOLD, &format!("▸{r}")),
                    std::cmp::Ordering::Greater => paint(DIM, r),
                })
                .collect::<Vec<_>>()
                .join(paint(DIM, " › ").as_str())
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
    // Said plainly either way: a cost of zero is usually an agent that does not
    // report one, and a `[budget]` ceiling over a silent agent is a guard that
    // never fires. Printing `$0.00` would hide exactly that.
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
    // Why it stopped, read from the row rather than guessed from the last
    // gate: four unrelated reasons reach `failed` and they want four different
    // sentences.
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
                "{} kept finding things and sent the work back to {} as often as allowed",
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

    // Preparing the checkout is not a check, and is shown as what it is.
    if let Some(setup) = w["setup"].as_object() {
        let ok = setup["commands"]
            .as_array()
            .map(|c| c.iter().all(|r| r["exit_code"] == 0))
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
                &format!("attempt {}", g["attempt"].as_u64().unwrap_or(1))
            ),
            if passed {
                paint(render::GREEN, "passed")
            } else {
                String::new()
            }
        );
        for cmd in g["commands"].as_array().unwrap_or(&empty) {
            let ok = cmd["exit_code"] == 0 && cmd["timed_out"] == false;
            println!(
                "  {} {}",
                if ok {
                    paint(render::GREEN, "✓")
                } else {
                    paint(render::RED, "✗")
                },
                cmd["command"].as_str().unwrap_or("")
            );
            for f in cmd["failures"].as_array().unwrap_or(&empty).iter().take(8) {
                println!("      {}", paint(DIM, f.as_str().unwrap_or("")));
            }
        }
    }

    // What the agent said, beside what the gate measured — and only there.
    //
    // An end-of-task report references about one action in eleven and drifts
    // toward the plan as the run leaves it, so it is worth very little alone
    // and is the whole point next to an exit code that contradicts it. This
    // prints the two and judges neither: no model separates a truthful
    // trajectory report from an untruthful one better than a bag-of-words
    // detector does, so the reader decides.
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

/// The issues one repository offers as work: `vibeplane issues --ready`.
///
/// Its own function rather than a `work` subcommand, because a reader should
/// not have to know which of two commands called `issues` answers which
/// question. The cross-project list lives in `cli::board`; this is the
/// label-filtered one `work start --issue` picks from.
pub async fn cmd_ready_issues(
    cwd: Option<std::path::PathBuf>,
    label: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let c = client::Client::connect_or_start().await?;
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
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
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
        paint(DIM, "vibeplane work start --issue <number> --kind bug")
    );
    Ok(())
}

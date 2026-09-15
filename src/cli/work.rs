//! Work: starting it, checking a project's configuration, and reading a verdict.

use super::WorkCmd;
use crate::render::{BOLD, DIM, YELLOW, clip, paint};
use crate::{client, render};
use anyhow::{Context, Result};
use std::path::PathBuf;

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

    let cache = crate::core::PolicyCache::for_projects_only();
    let verdict = cache.evaluate(&dir, &tool, &input);
    let restrictive = cache.restrictive(&dir, &tool, &input);
    let root = crate::core::project::find_repo_root(&dir).unwrap_or_else(|| dir.clone());
    let problems = crate::core::ProjectConfig::load(&root)
        .map(|c| c.validate())
        .unwrap_or_default();

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
        None => println!(
            "        {}",
            paint(
                DIM,
                "no rule answers this one, so the provider's own dialog decides \
                 and it reaches your inbox"
            )
        ),
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
                "problems": problems,
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
        for rule in policy.deny_rules() {
            println!("            {} {}", paint(render::RED, "deny "), rule);
        }
        for rule in policy.ask_rules() {
            println!("            {} {}", paint(render::YELLOW, "ask  "), rule);
        }
        for rule in policy.allow_rules() {
            println!("            {} {}", paint(render::GREEN, "allow"), rule);
        }
        // Advice, printed once and not per rule. A `Read` deny does exactly
        // what it says; what it does not say — that a shell redirection and
        // `touch` are `Edit` business — is the part people get wrong, and
        // `Read(.env)` is the most common rule anybody writes. As a per-rule
        // warning this would fire on the canonical example and teach people to
        // ignore warnings.
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

pub async fn cmd_trust(path: PathBuf, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let root = path.canonicalize().context("that path does not exist")?;
    let v: serde_json::Value = c
        .post_json(
            "/api/projects/trust",
            &serde_json::json!({ "path": root.to_string_lossy() }),
        )
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
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

        WorkCmd::Issues { cwd, label } => {
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
}

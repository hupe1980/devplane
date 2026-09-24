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
                &format!("devplane say {} ...", v["run_id"].as_str().unwrap_or(""))
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
    /// No `devplane.toml` governs this directory at all.
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

/// `devplane explain` — what the gate would decide about one call, and why.
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
    // `~/.devplane/policy.toml`, which let the surface built to say *what
    // would the gate decide* answer `allow` for a call the machine denies.
    let (cache, global_error) = crate::core::PolicyCache::from_disk();
    // One evaluator, so this surface cannot print two answers to one question.
    // It used to call `evaluate` and `restrictive` and label them *the verdict*
    // and *what a `PreToolUse` hook would answer* — two names over one value,
    // from the days when only one of them could return `allow`.
    let verdict = cache.restrictive(&dir, &tool, &input);

    // **Where the rules came from, and whether they loaded.** `undecided` used
    // to be one sentence covering three different situations, and only one of
    // them is a fact about the call: no rules here, rules that would not load,
    // and rules that loaded and did not match. The middle one is the dangerous
    // one — a `devplane.toml` with a typo in it has *no* rules in force, and
    // reporting that as "no rule answers this one" is a typo in a deny rule
    // reading as permission, which is the one thing this layer may never do.
    //
    // `devplane check` has always printed the parse error. This is the surface
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
                // Set only for `unresolved`, where there is no rule to name and
                // this sentence is the whole of the answer.
                "why": verdict.why(),
                // Which of the three `undecided` situations this is.
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
        // No rule, but a reason: the matcher could not read the command and a
        // prohibition here is about what runs. Printing `why_nothing_answered`
        // would say "no rule covers this", which is the false half.
        (None, Some(why)) => println!("        {}", paint(DIM, &format!("because {why}"))),
        (None, None) => println!("        {}", paint(DIM, rules.why_nothing_answered())),
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
                "every rule in this file is off until it parses — devplane check"
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

/// Replay every tool call this machine has seen against the rules as they are
/// now, and say which rule would stop the interruptions.
///
/// The product measures its own inbox (`devplane attention`); this is the
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

    // **The machine-wide file included**, which this command did not do. The
    // `for_projects_only` constructor says in its own doc that it is for tests
    // and that `devplane explain` using it was a bug — the fix reached
    // `explain` and not `explain --replay`, so the replay answered "no rule
    // here" for every call `~/.devplane/policy.toml` forbids.
    let (cache, _) = crate::core::PolicyCache::from_disk();
    // The third is every call no rule here decides — which is most of them, and
    // it is *not* the same as "interrupted a person": the agent's own settings
    // answered most of these silently. This counter said "reached you" until
    // the approval path was deleted made the difference visible, and a count
    // that overstates how often somebody was interrupted is an argument for
    // writing rules that were never needed.
    let mut counts = [0usize; 3]; // ask, deny, no rule here
    // Commands that reached a person, grouped by the rule that would answer
    // them. `BTreeMap` so two runs of this print the same thing.
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
/// **The verdict half of `check`.** `devplane check` answers *what will this
/// file do*; this answers *what did the commands in it just say*. It exists
/// because something outside this process asks the question — a Spec Kit
/// workflow tells an agent to run it and report the result — so the answer has
/// to be legible to a reader that is not a person, and has to distinguish
/// *nothing was checked* from *the checks said no*.
///
/// It decides on exit codes and nothing else. No specification is opened, no
/// task list is parsed, no prose is graded.
pub async fn cmd_gate_run(cwd: Option<PathBuf>, name: Option<String>, json: bool) -> Result<()> {
    use crate::core::work::GateState;

    let here = match cwd {
        Some(p) => p.canonicalize().context("that path does not exist")?,
        None => std::env::current_dir()?,
    };
    let root = crate::core::project::find_repo_root(&here).unwrap_or(here);

    // Three outcomes before a command is run, and each is its own sentence.
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
            // Not a pass, and not silently a failure of the checks either: the
            // checks did not run.
            std::process::exit(1);
        }
        Ok(c) => c,
    };

    // **A named gate, where one was asked for.** `check` is the definition of
    // done and always resolves; anything else has to be declared, and asking for
    // one that is not is an error rather than a silent pass — an empty gate that
    // reads as success is the failure this layer exists to prevent.
    let (label, commands, expect, timeout) = match name.as_deref() {
        None => (
            "check",
            config.gates.check.clone(),
            crate::core::config::Expect::Pass,
            config.gates.timeout,
        ),
        Some(n) => match config.gate_named(n) {
            Some((run, expect, timeout)) => (n, run, expect, timeout),
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
                std::process::exit(1);
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
        std::process::exit(1);
    }

    let report = crate::gates::run(label, &commands, &root, timeout, 1).await;
    // **A named gate may be declared `expect = "fail"`**, and a reader that
    // ignored that would call a gate which did exactly what it was asked to do
    // a failure. `check` is always `Pass`, so this is a no-op for it.
    let passed = match expect {
        crate::core::config::Expect::Pass => report.passed(),
        crate::core::config::Expect::Fail => !report.passed(),
    };
    let state = match passed {
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
                // The gate's own sentence, which names what failed; the state's
                // is the frame. Both, because a reader outside this process has
                // neither by default.
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
        // **Said, because the alternative is a reader assuming otherwise.**
        // This runs in a repository rather than against a piece of work, so
        // there is nothing for the row to be about and no daemon in the path.
        // A verdict that is on the record is what `devplane work verify` is
        // for, and pointing at it is more honest than inventing a way to append
        // to the audit log from any client holding the token.
        println!(
            "{}",
            paint(
                DIM,
                "Not recorded: this ran in a repository rather than against a piece of work. \
                 `devplane work verify <id>` is the one that leaves a row."
            )
        );
    }

    match state.passed() {
        true => Ok(()),
        false => std::process::exit(1),
    }
}

/// Registers Devplane's gate as a Spec Kit extension hook.
///
/// **It writes the file only when there is none, and otherwise prints what to
/// add.** That is not timidity; it is the same rule the permission-rule offer
/// follows, and it is stronger than the alternative here. `extensions.yml` is a
/// committed file that may carry other people's hooks, comments and key order,
/// and the only way to add an entry without a YAML parser is to append text to
/// a structure this tool has not understood. Round-tripping it through a parser
/// would preserve the entries and destroy the comments.
///
/// So: an absent file is written in full, because there is nothing to damage. A
/// present one is left alone and the person is handed the four lines and the
/// key they go under — which they can read, review and commit like the code it
/// is.
pub fn cmd_speckit_install(event: Option<String>, dry_run: bool, anyway: bool) -> Result<()> {
    use crate::core::spec::{DEFAULT_HOOK_EVENT, EXTENSIONS_FILE, HOOK_COMMAND, HOOK_EVENTS};

    let here = std::env::current_dir()?;
    let root = crate::core::project::find_repo_root(&here).unwrap_or(here);

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

    // **A mandatory hook is one the agent must invoke and wait for**, so
    // registering one whose command nothing defines does not degrade to no gate:
    // the workflow reaches a step it may not skip and cannot perform. Checked
    // before anything is written, and refused rather than warned, because the
    // failure is silent from inside — the same shape as a deny rule that matches
    // nothing.
    if !anyway
        && let Some(why) = crate::core::spec::unreachable_skill(&root, dirs::home_dir().as_deref())
    {
        let places = crate::core::spec::skill_locations(&root, dirs::home_dir().as_deref());
        anyhow::bail!(
            "{why}.\n\n\
             A hook registered `optional: false` is one the agent is told it may not skip. \n\
             Registering it now would put a step in the workflow that cannot run.\n\n\
             Two ways to make it reachable:\n  \
             · install the plugin — `claude plugin marketplace add hupe1980/devplane`, then \
             `/plugin install devplane@devplane`\n  \
             · or put a skill of that name at one of:\n      {}\n\n\
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

    // A file that already names the command is one somebody registered. Said as
    // an observation rather than a certainty: this reads the text rather than
    // the structure, which is exactly why it does not then edit it.
    if let Some(text) = &existing
        && text.contains(HOOK_COMMAND)
    {
        println!(
            "{} already names `{HOOK_COMMAND}`. Nothing to do.",
            EXTENSIONS_FILE
        );
        return Ok(());
    }

    let entry = crate::core::spec::hook_entry();
    match existing {
        None => {
            println!("{EXTENSIONS_FILE} would be created, containing:\n");
            println!("{}", crate::core::spec::extensions_file(&event));
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
            std::fs::write(&path, crate::core::spec::extensions_file(&event))?;
            println!("{}  {EXTENSIONS_FILE}", paint(render::GREEN, "written"));
        }
        Some(_) => {
            // **Never edited.** See the doc comment: a file this tool has not
            // parsed is a file it does not rewrite.
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
             Add a devplane.toml when you want gates.",
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
    // **What happens to a question nobody answers**, which is a decision this
    // file takes on somebody's behalf and is therefore exactly what `check`
    // exists to print. A deadline that ends a question is invisible until it
    // fires, and by then the agent has been told no in the reader's name.
    //
    // A value that will not parse is a **problem**, not a default: the
    // difference between `4h` and a typo is the difference between a bounded
    // wait and an unbounded one, and a file that says something unreadable
    // about somebody's questions should say so out loud here.
    // A value that will not parse is already an error in the problem list
    // above, so this line states the deadline and does not repeat the
    // complaint: one fact, one place, and a reader who has just been told the
    // file is wrong does not need telling twice in different words.
    if let Some(d) = config.questions.deadline() {
        println!("  questions {}", crate::core::ask::Deadline::says(d));
    }

    // **The words this repository uses for a question its plan has not
    // answered**, read back for the same reason the deadline is: it decides
    // whether an inbox item ever appears, and a key that is silently empty is
    // indistinguishable from a specification with nothing outstanding.
    //
    // Printed only where the repository set some. Saying "none" to every
    // project that does not work this way would be noise in the one command
    // people run to find out what a file actually does.
    // **Where this repository keeps its plans**, read back beside the words.
    // Both decide whether a surface shows anything, and `plans` decides it
    // hardest: without it the Plans page says nothing about this project at
    // all, which is indistinguishable from a repository that has no plans.
    if let Some(dir) = config.spec.plans.as_deref() {
        let n = config.spec.plan_paths(&root).len();
        println!("  {:<9} plans in {dir} ({n} found)", "spec");
    }
    if !config.spec.open_questions.is_empty() {
        // The label is padded by a width rather than by spaces in the literal:
        // a run of spaces inside a string is what a dropped `\` continuation
        // looks like, and `purity` refuses them for that reason.
        println!(
            "  {:<9} unresolved when a line says: {}",
            "spec",
            config.spec.open_questions.join(", ")
        );
    }

    // What the rules actually do, which is the half of "is this file right"
    // that the problem list cannot answer. A rule that parses, is legal and
    // still covers nothing anybody expected is only visible by reading it back.
    let policy = config.policy();
    if !policy.is_empty() {
        println!(
            "  policy    {} deny, {} ask",
            policy.deny_rules().len(),
            policy.ask_rules().len()
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
        for finding in &policy.redundancies() {
            let mut lines = crate::core::text::wrap(finding, 66).into_iter();
            if let Some(first) = lines.next() {
                println!("  {}{}", render::pad(&paint(BOLD, "unused"), 10), first);
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
        // Rules in the agent's own allow list that approve nothing at all — a
        // negation, an unanchored tool-name glob, a parameter rule. Reported
        // for the reason `overbroad` is, and it is the other half of the same
        // question: one is a rule that grants more than it reads as granting,
        // this is a rule that grants nothing while reading as permission.
        //
        // These checks existed and could never run: they live on the allow
        // side of `Rule::problems`, and nothing has compiled an allow list
        // since approving was deleted.
        let inert = crate::core::policy::allow_rules_that_grant_nothing(&vendor_allow_rules(
            root.as_path(),
        ));
        if !inert.is_empty() {
            println!();
        }
        for what in &inert {
            let mut lines = crate::core::text::wrap(what, 66).into_iter();
            if let Some(first) = lines.next() {
                println!("  {}{}", render::pad(&paint(BOLD, "grants no"), 10), first);
            }
            for rest in lines {
                println!("  {:<10}{}", "", rest);
            }
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

/// `devplane trust` — show what a repository's agent configuration does, then
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
                    // What is there, beside what is worth flagging: a finding
                    // list of zero does not mean the repository ships nothing.
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
        // What is *there* and what is worth flagging are different questions,
        // and this line used to answer the second while appearing to answer the
        // first — it said "no skills" about a repository shipping ten that
        // happen to pre-approve nothing.
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
                    "say what the work is: devplane work start \"fix the flaky test\"\n\
                     or point at an issue:  devplane work start --issue 7"
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
                        "devplane work verify {}",
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

        WorkCmd::Export { work } => {
            let v: serde_json::Value = c
                .get(&format!("/api/work/{work}/certificate"))
                .await
                .with_context(|| format!("no work `{work}`"))?;
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("{e}");
            }
            // Unfinished work is not an error: the reply says where it is, and
            // saying so is the honest answer to a fair question.
            match json {
                true => println!("{}", serde_json::to_string_pretty(&v["statement"])?),
                false => println!("{}", v["markdown"].as_str().unwrap_or("")),
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
                    paint(BOLD, "devplane work start \"fix the flaky login test\"")
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
                // **The plan, beside the verdict.** *Done, and the specification
                // it answers has eleven boxes unticked* is a sentence no exit
                // code and no self-report can produce alone — and until this
                // line it could only be reached by exporting a certificate,
                // after the approval it would have changed.
                //
                // The daemon composes the judgement; this renders it. A second
                // copy of *what counts as a contradiction* in the terminal is
                // the shape that got the reproduction gate backwards.
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
            // **A tick and a cross cannot say four things.** A command that
            // exited non-zero, one that ran out of time, one the shell never
            // started and one whose result could not be collected are four
            // different sentences, and only the first is a verdict about the
            // work. The glyph carries the first distinction and the word beside
            // it carries the rest.
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

/// The issues one repository offers as work: `devplane issues --ready`.
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
        paint(DIM, "devplane work start --issue <number> --kind bug")
    );
    Ok(())
}

/// The rules that actually grant, which are the agent's and not this product's.
///
/// `devplane.toml` has no allow list: nothing here approves, so a grant written
/// here would approve nothing and warning about it would be theatre. The file
/// that does grant is the agent's own, and it is the one worth reading.
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

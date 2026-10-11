//! Steering a change while it is in flight: verify, retry, resume, prompt,
//! stop, and deciding about a specification that moved.

use super::{Answer, Says, host_and_change, ids};
use crate::render::{self, DIM, clip, paint};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de::IgnoredAny;

/// What `verify` came to: the gates' word, and the base checks when asked.
#[derive(Deserialize)]
struct Verified {
    #[serde(default)]
    passed: bool,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    report: Option<crate::core::change::GateReport>,
    #[serde(default)]
    base_checks: Option<crate::core::base_checks::View>,
    #[serde(default)]
    base_checks_refused: Option<String>,
}

/// The specification an accepted drift now works to.
#[derive(Deserialize)]
struct Accepted {
    #[serde(default)]
    fingerprint: Option<String>,
}

pub(super) async fn cmd_verify(change: &str, base_checks: bool, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    if !json {
        println!("{}", paint(DIM, "running the project's gates…"));
    }
    let answer: Answer<Verified> = c
        .post_json(
            &format!("/api/changes/{id}/verify"),
            &serde_json::json!({ "base_checks": base_checks }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    let passed = v.passed;
    println!(
        "{} {}",
        match passed {
            true => paint(render::GREEN, "passed"),
            false => paint(render::RED, "failed"),
        },
        v.summary.as_deref().unwrap_or("")
    );
    // Every sentence is the host's; a count is not a verdict, so none is coloured.
    let b = v.base_checks.as_ref();
    if let Some(why) = &v.base_checks_refused {
        println!("base checks  {why}");
    }
    for (label, says) in [
        (
            "base's tests on this change",
            b.and_then(|b| b.on_change.as_ref()).map(|o| &o.says),
        ),
        (
            "new tests on the base",
            b.and_then(|b| b.on_base.as_ref()).map(|o| &o.says),
        ),
    ] {
        if let Some(says) = says {
            println!("  {label}  {says}");
        }
    }
    if let Some(b) = b {
        println!("tests      {}", b.counts_says);
    }
    if !passed {
        for cmd in v.report.iter().flat_map(|r| r.commands.iter()) {
            for f in cmd.failures.iter().take(10) {
                println!("    {f}");
            }
        }
    }
    Ok(())
}

pub(super) async fn cmd_retry(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let answer: Answer<IgnoredAny> = c
        .post_json(&format!("/api/changes/{id}/retry"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    answer.read()?;
    println!(
        "{} the failures went back to the same session.",
        paint(render::GREEN, "handed back —")
    );
    Ok(())
}

/// What a resume came to: the same session, or another agent's new run.
#[derive(Deserialize)]
struct Resumed {
    #[serde(default)]
    switched: Option<Switched>,
}

#[derive(Deserialize)]
struct Switched {
    run: String,
    agent: String,
    #[serde(default)]
    was: Option<String>,
}

pub(super) async fn cmd_resume(
    change: &str,
    agent: Option<&str>,
    prompt: &str,
    json: bool,
) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let body = match agent {
        Some(a) => serde_json::json!({ "agent": a, "prompt": prompt }),
        None => serde_json::json!({}),
    };
    let answer: Answer<Resumed> = c
        .post_json(&format!("/api/changes/{id}/resume"), &body)
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    match &answer.read()?.switched {
        Some(s) => println!(
            "{} continued {id} with {} (was {}) — run {}",
            paint(render::GREEN, "switched —"),
            s.agent,
            s.was.as_deref().unwrap_or("no agent"),
            s.run
        ),
        None => println!(
            "{} the agent is back on the same conversation.",
            paint(render::GREEN, "resumed —")
        ),
    }
    Ok(())
}

/// A run as `login` reads it: how the agent was started, where, and the
/// sign-in methods it offered.
#[derive(Deserialize)]
struct SignInRun {
    #[serde(default)]
    agent_command: Option<String>,
    cwd: std::path::PathBuf,
    #[serde(default)]
    sign_in: Option<Vec<crate::core::event::SignInMethod>>,
}

pub(super) async fn cmd_login(change: &str, method: Option<&str>, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let run = run_of(&c, &id).await?;
    let read: Answer<SignInRun> = c.get(&format!("/api/runs/{run}")).await?;
    let run_doc = read.read()?;
    let methods = run_doc
        .sign_in
        .clone()
        .with_context(|| format!("change {id}'s agent did not say it needs signing in"))?;
    let Some(method) = method else {
        if json {
            return crate::cli::print_json(&serde_json::json!({ "methods": methods }));
        }
        println!("The agent offers these ways to sign in, in its own words:");
        for m in &methods {
            let what = match m.kind.as_str() {
                "terminal" => "runs the agent's own login in this terminal",
                "agent" => "the agent signs itself in when asked",
                _ => "a kind Devplane does not act on",
            };
            println!("  {}  \"{}\" — {what}", m.id, clip(&m.name, 60));
            if let Some(d) = &m.description {
                println!("      {}", paint(DIM, &format!("\"{}\"", clip(d, 100))));
            }
        }
        println!("devplane change login {id} --method <id>");
        return Ok(());
    };
    let chosen = methods
        .iter()
        .find(|m| m.id == method)
        .with_context(|| format!("the agent offered no sign-in method `{method}`"))?;
    let after_terminal = match chosen.kind.as_str() {
        "agent" => false,
        "terminal" => {
            let command = run_doc
                .agent_command
                .as_deref()
                .context("the run does not record how its agent was started")?;
            println!(
                "{}",
                paint(
                    DIM,
                    &format!(
                        "running the agent's own login: {command} {}",
                        chosen.args.join(" ")
                    )
                )
            );
            let status = terminal_login(command, &chosen.args, &chosen.env, &run_doc.cwd)?;
            if !status.success() {
                anyhow::bail!("the agent's login exited with {status}; nothing was started");
            }
            true
        }
        other => anyhow::bail!(
            "`{}` is a sign-in of a kind Devplane does not know (`{other}`), so it does not act on it",
            chosen.name
        ),
    };
    let answer: Answer<Started> = c
        .post_json(
            &format!("/api/changes/{id}/login"),
            &serde_json::json!({ "method": method, "after_terminal": after_terminal }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    println!(
        "{} run {}",
        paint(render::GREEN, "started again —"),
        answer.read()?.run
    );
    Ok(())
}

/// The run a sign-in started.
#[derive(Deserialize)]
struct Started {
    run: String,
}

/// Runs the agent's command plus a terminal sign-in method's arguments and
/// variables, attached to this terminal. Devplane reads nothing it prints.
fn terminal_login(
    command: &str,
    args: &[String],
    env: &std::collections::BTreeMap<String, String>,
    dir: &std::path::Path,
) -> Result<std::process::ExitStatus> {
    let quoted: Vec<String> = args
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
        .collect();
    let line = format!("{command} {}", quoted.join(" "));
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", &line]);
        c
    } else {
        let mut c = std::process::Command::new("/bin/sh");
        c.args(["-c", &line]);
        c
    };
    cmd.current_dir(dir)
        .envs(env)
        .status()
        .context("starting the agent's login")
}

/// The run a change-or-run argument addresses: a change through its latest run.
async fn run_of(c: &crate::client::Client, needle: &str) -> Result<String> {
    let reader = crate::local::Reader::Host(c.clone());
    match ids::change_or_run(&reader, needle).await? {
        ids::Subject::Run(run) => Ok(run),
        ids::Subject::Change(id) => reader
            .change(&id)
            .await?
            .change
            .runs
            .last()
            .map(|r| r.as_str().to_string())
            .with_context(|| format!("change {id} has no run to address")),
    }
}

pub(super) async fn cmd_prompt(run: &str, text: Vec<String>, json: bool) -> Result<()> {
    let text = text.join(" ");
    if text.trim().is_empty() {
        anyhow::bail!("say what to send: devplane change prompt <change|run> \"use the helper\"");
    }
    let c = crate::client::Client::connect_running().await?;
    let run = run_of(&c, run).await?;
    let answer: Answer<Says> = c
        .post_json(
            &format!("/api/runs/{run}/prompt"),
            &serde_json::json!({ "text": text }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    println!("{}", answer.read()?.says.as_deref().unwrap_or("sent"));
    Ok(())
}

pub(super) async fn cmd_stop(run: &str, yes: bool, json: bool) -> Result<()> {
    let c = crate::client::Client::connect_running().await?;
    let run = run_of(&c, run).await?;
    // What survives, said before anything is stopped.
    let before: Answer<Says> = c.get(&format!("/api/runs/{run}/stop")).await?;
    let says = before.read()?.says.as_deref().unwrap_or("");
    match json {
        true => eprintln!("{says}"),
        false => println!("{says}"),
    }
    if !crate::cli::confirm("Stop it?", yes)? {
        eprintln!("{}", paint(DIM, "not stopped"));
        return Ok(());
    }
    let answer: Answer<IgnoredAny> = c
        .post_json(&format!("/api/runs/{run}/stop"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    answer.read()?;
    println!("{}", paint(render::GREEN, "stopped"));
    Ok(())
}

pub(super) async fn cmd_drift(
    change: &str,
    run: &str,
    accept: bool,
    tell: bool,
    json: bool,
) -> Result<()> {
    let verb = match (accept, tell) {
        (true, false) => "accept",
        (false, true) => "tell",
        _ => anyhow::bail!("say which: devplane change drift <id> --run <run> --accept | --tell"),
    };
    let (c, id) = host_and_change(change).await?;
    let run = ids::run(&crate::local::Reader::Host(c.clone()), run).await?;
    let answer: Answer<Accepted> = c
        .post_json(
            &format!("/api/changes/{id}/drift/{verb}"),
            &serde_json::json!({ "run": run }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    match verb {
        "accept" => println!(
            "{} the change now works to the specification run {} saw ({}); recorded as yours.",
            paint(render::GREEN, "accepted —"),
            clip(&run, 12),
            clip(v.fingerprint.as_deref().unwrap_or(""), 12)
        ),
        _ => println!(
            "{} run {} was handed the files that changed; recorded as yours.",
            paint(render::GREEN, "told —"),
            clip(&run, 12)
        ),
    }
    Ok(())
}

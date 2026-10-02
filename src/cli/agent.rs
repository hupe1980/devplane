//! `devplane agent`: the agents Devplane can drive, and installing or removing
//! its hooks in a vendor's own settings.

use super::Vendor;
use crate::observe::connect::{read_existing, write_whole};
use crate::render::{BOLD, DIM, clip, paint};
use crate::{config, render};
use anyhow::{Context, Result};

pub async fn cmd_agents(json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let v: serde_json::Value = c.get("/api/agents").await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = vec![];
    let mut any_measured = false;
    for a in v.as_array().unwrap_or(&empty) {
        println!(
            "{:<10} {:<14} {}",
            paint(BOLD, a["id"].as_str().unwrap_or("")),
            a["name"].as_str().unwrap_or(""),
            paint(DIM, a["command"].as_str().unwrap_or(""))
        );
        // Capabilities measured at `initialize`, only for agents Devplane has
        // started: "not probed" is not "not supported".
        if let Some(m) = a.get("measured").filter(|m| !m.is_null()) {
            any_measured = true;
            let yes = |k: &str| m.get(k).and_then(|b| b.as_bool()).unwrap_or(false);
            let mut can: Vec<&str> = Vec::new();
            if yes("resume") {
                can.push("resume");
            }
            if yes("load_session") {
                can.push("load");
            }
            if yes("list_sessions") {
                can.push("list");
            }
            if yes("declares_modes") {
                can.push("modes");
            }
            if yes("needs_auth") {
                can.push("needs sign-in");
            }
            let when = m
                .get("at")
                .and_then(|t| t.as_str())
                .map(|t| clip(t, 10))
                .unwrap_or_default();
            println!(
                "           {}",
                paint(
                    DIM,
                    &format!(
                        "{} · measured {when}",
                        if can.is_empty() {
                            "nothing beyond the baseline".to_string()
                        } else {
                            can.join(" · ")
                        }
                    )
                )
            );
        }
    }
    if !any_measured {
        println!(
            "\n{}",
            paint(
                DIM,
                "No agent has been started yet, so nothing here is measured.\nWhat an agent supports is advertised when it starts, not listed in a registry."
            )
        );
    }
    println!(
        "\n{}\n{}",
        paint(
            DIM,
            "Add one by name in ~/.devplane/agents.toml:  [agents.kimi] command = \"...\""
        ),
        paint(
            DIM,
            "Or point at a command: devplane change start --agent '/opt/my-agent --acp' ..."
        )
    );
    Ok(())
}

pub async fn cmd_connect(
    what: Vendor,
    statusline: bool,
    yes: bool,
    json: bool,
    bin: Option<std::path::PathBuf>,
) -> Result<()> {
    // Every hook runs this path; one that vanishes turns the gate off.
    let exe = crate::observe::connect::stable_binary(bin)?;
    if what == Vendor::Copilot {
        return connect_copilot(yes, json, &exe).await;
    }
    if what == Vendor::Codex {
        return connect_codex(yes, json, &exe).await;
    }
    // The running host's endpoint, or the default port. Nothing is started:
    // the gate works whether or not anything is listening.
    let base_url = host_base_url();
    let token = config::ingest_token(&config::load_or_create_token()?);
    let path = crate::observe::connect::settings_path()?;
    let mut settings = crate::observe::connect::read_settings(&path)?;
    let before = crate::observe::connect::render_settings(&settings);
    let mut report = crate::observe::connect::connect(&mut settings, &base_url, &token, &exe);
    if statusline {
        report.notes.push(crate::observe::connect::wrap_status_line(
            &mut settings,
            &exe,
        ));
    }
    let after = crate::observe::connect::render_settings(&settings);
    let diff = crate::observe::connect::diff(&before, &after);

    if json {
        // `--json` is the scripted form: the diff is reported, and the write
        // happens only when `--yes` says so.
        require_yes(yes, !diff.is_empty(), &path)?;
        if !diff.is_empty() {
            crate::observe::connect::write_settings(&path, &settings)?;
        }
        let mut out = serde_json::to_value(&report)?;
        out["settings_path"] = serde_json::json!(path.display().to_string());
        out["diff"] = serde_json::json!(diff);
        out["written"] = serde_json::json!(!diff.is_empty());
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if diff.is_empty() {
        println!(
            "{} {} already has exactly this; nothing to write",
            paint(DIM, "unchanged"),
            path.display()
        );
    } else {
        if !yes && !show_diff_and_confirm(&path, &diff)? {
            println!("{}", paint(DIM, "left as it was"));
            return Ok(());
        }
        crate::observe::connect::write_settings(&path, &settings)?;
        println!(
            "{} {} hook events into {}",
            paint(render::GREEN, "installed"),
            report.hooks_added,
            path.display()
        );
    }
    match report.telemetry {
        crate::observe::connect::TelemetryStatus::Configured => {
            println!(
                "{} telemetry → {}",
                paint(render::GREEN, "configured"),
                base_url
            )
        }
        crate::observe::connect::TelemetryStatus::External => println!(
            "{} telemetry already goes elsewhere; left alone",
            paint(render::YELLOW, "skipped")
        ),
        _ => {}
    }
    for n in &report.notes {
        println!("  {}", paint(DIM, n));
    }
    println!(
        "\n{}\n  Existing sessions pick the hooks up on their next turn.\n  New sessions are seen immediately. Try {}.",
        paint(BOLD, "Done."),
        paint(BOLD, "devplane status")
    );
    Ok(())
}

/// Prints the change a connect or disconnect is about to make to a file the
/// person owns, and asks. On a terminal a non-answer is no; with stdin not a
/// terminal nothing is written without `--yes`.
fn show_diff_and_confirm(path: &std::path::Path, diff: &str) -> Result<bool> {
    println!("{} {}", paint(BOLD, "Changes to"), path.display());
    for line in diff.lines() {
        let painted = match line.as_bytes().first() {
            Some(b'+') => paint(render::GREEN, line),
            Some(b'-') => paint(render::RED, line),
            _ => paint(DIM, line),
        };
        println!("  {painted}");
    }
    // Not `--yes` here: the caller skips this whole function when it is set.
    super::confirm("Write this?", false)
}

/// Refuses a scripted (`--json`) write nobody confirmed.
fn require_yes(yes: bool, would_write: bool, path: &std::path::Path) -> Result<()> {
    if would_write && !yes {
        anyhow::bail!(
            "nothing was written to {}: under --json nobody can confirm the change. Re-run \
             with `--yes` to write it (the diff is shown without --json).",
            path.display()
        );
    }
    Ok(())
}

/// `devplane agent connect copilot`.
///
/// Copilot loads every `*.json` in `~/.copilot/hooks/`, so the installation is
/// one file Devplane owns, and every hook in it runs this binary. Telemetry
/// cannot be switched on from here (it lives in the environment or managed
/// settings), so the environment lines are printed for the person to run.
async fn connect_copilot(yes: bool, json: bool, exe: &std::path::Path) -> Result<()> {
    let base_url = host_base_url();
    let token = config::ingest_token(&config::load_or_create_token()?);
    // The file is entirely ours: diffed against what was written last time,
    // the same plan the window shows.
    let planned = crate::observe::connect::plan_connect(
        crate::observe::connect::Agent::Copilot,
        exe,
        &base_url,
        &token,
    )?;
    let (file, after, diff) = (
        planned.path,
        planned.after.unwrap_or_default(),
        planned.diff,
    );

    // Three lines, not two: the telemetry routes need a bearer token — the
    // telemetry-only one, since an agent can read its own environment.
    let env = [
        ("COPILOT_OTEL_ENABLED", "true".to_string()),
        (
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            format!("{}/devplane/otel", base_url),
        ),
        (
            "OTEL_EXPORTER_OTLP_HEADERS",
            format!("Authorization=Bearer {token}"),
        ),
    ];
    if json {
        if !diff.is_empty() {
            write_whole(&file, &after)?;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hooks_file": file.display().to_string(),
                "diff": diff,
                "written": !diff.is_empty(),
                "telemetry": "manual",
                "env": env.iter().map(|(k, v)| (k.to_string(), v.clone()))
                    .collect::<std::collections::BTreeMap<_, _>>(),
            }))?
        );
        return Ok(());
    }
    if diff.is_empty() {
        println!(
            "{} {} already has exactly this",
            paint(DIM, "unchanged"),
            file.display()
        );
    } else {
        if !yes && !show_diff_and_confirm(&file, &diff)? {
            println!("{}", paint(DIM, "left as it was"));
            return Ok(());
        }
        write_whole(&file, &after)?;
        println!("{} {}", paint(render::GREEN, "wrote"), file.display());
    }
    println!(
        "  {}",
        paint(
            DIM,
            "every hook runs the devplane binary; the permission gate is the one whose answer is read"
        )
    );
    println!(
        "
{} Copilot reads telemetry from the environment, so these lines are yours to add:
",
        paint(BOLD, "One step left.")
    );
    for (k, v) in &env {
        println!("  export {k}={v}");
    }
    println!(
        "
  {}",
        paint(
            DIM,
            "Devplane does not edit shell profiles, and its managed-settings equivalent \
             belongs to your organisation rather than to this tool."
        )
    );
    println!(
        "
{} Sessions started after this are seen. There is no roster for Copilot, so
           nothing appears until a session does something.",
        paint(BOLD, "Done.")
    );
    Ok(())
}

pub async fn cmd_disconnect(what: Vendor, yes: bool, json: bool) -> Result<()> {
    if what == Vendor::Codex {
        return disconnect_codex(yes, json).await;
    }
    if what == Vendor::Copilot {
        let file = crate::observe::copilot::hooks_dir()
            .map(|d| d.join(crate::observe::copilot::HOOKS_FILE))
            .filter(|f| f.exists());
        // The whole file is ours, so removing it is the diff.
        let diff = file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .map(|before| crate::observe::connect::diff(&before, ""))
            .unwrap_or_default();
        if json {
            if let Some(f) = &file {
                require_yes(yes, true, f)?;
                std::fs::remove_file(f).ok();
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "removed": file.as_ref().map(|f| f.display().to_string()),
                    "diff": diff,
                }))?
            );
            return Ok(());
        }
        match file {
            Some(f) => {
                if !yes && !show_diff_and_confirm(&f, &diff)? {
                    println!("{}", paint(DIM, "left as it was"));
                    return Ok(());
                }
                std::fs::remove_file(&f).with_context(|| format!("removing {}", f.display()))?;
                println!("{} {}", paint(render::GREEN, "removed"), f.display());
            }
            None => println!("{}", paint(DIM, "nothing of ours was installed")),
        }
        println!(
            "  {}",
            paint(DIM, "any OTel variables you exported are yours to remove")
        );
        return Ok(());
    }

    let path = crate::observe::connect::settings_path()?;
    let mut settings = crate::observe::connect::read_settings(&path)?;
    let before = crate::observe::connect::render_settings(&settings);
    let report = crate::observe::connect::disconnect(&mut settings);
    let after = crate::observe::connect::render_settings(&settings);
    let diff = crate::observe::connect::diff(&before, &after);
    if json {
        require_yes(yes, !diff.is_empty(), &path)?;
        if !diff.is_empty() {
            crate::observe::connect::write_settings(&path, &settings)?;
        }
        let mut out = serde_json::to_value(&report)?;
        out["settings_path"] = serde_json::json!(path.display().to_string());
        out["diff"] = serde_json::json!(diff);
        out["written"] = serde_json::json!(!diff.is_empty());
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if diff.is_empty() {
        println!(
            "{} nothing of ours in {}",
            paint(DIM, "unchanged"),
            path.display()
        );
        return Ok(());
    }
    if !yes && !show_diff_and_confirm(&path, &diff)? {
        println!("{}", paint(DIM, "left as it was"));
        return Ok(());
    }
    crate::observe::connect::write_settings(&path, &settings)?;
    println!(
        "{} {} hook entries from {}",
        paint(render::GREEN, "removed"),
        report.hooks_removed,
        path.display()
    );
    for n in &report.notes {
        println!("  {}", paint(DIM, n));
    }
    Ok(())
}

/// `devplane agent connect codex`: Devplane's entries merged into `~/.codex/hooks.json`.
///
/// The file is the person's, so entries are merged in and picked back out on
/// disconnect. No telemetry (none is documented). Codex silently skips any
/// hook the person has not approved in its own dialog, so that is said last.
async fn connect_codex(yes: bool, json: bool, exe: &std::path::Path) -> Result<()> {
    // The same plan the window shows: Devplane's entries merged in.
    let planned =
        crate::observe::connect::plan_connect(crate::observe::connect::Agent::Codex, exe, "", "")?;
    let (file, after, diff) = (
        planned.path,
        planned.after.unwrap_or_default(),
        planned.diff,
    );
    let installed = |text: &str| {
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(text)
            .map(|d| crate::observe::codex::installed(&d).len())
            .unwrap_or(0)
    };
    // How many hook events are registered, as `install` counts them.
    let added = installed(&after);
    let trust = "Codex runs a hook only after you approve it in its own dialog, and skips one \
                 you have not without a word — until then nothing here is watched or gated";
    if json {
        require_yes(yes, !diff.is_empty(), &file)?;
        if !diff.is_empty() {
            write_whole(&file, &after)?;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hooks_file": file.display().to_string(),
                "hooks_added": added,
                "diff": diff,
                "written": !diff.is_empty(),
                "notes": [trust],
            }))?
        );
        return Ok(());
    }
    if diff.is_empty() {
        println!(
            "{} {} already has exactly this",
            paint(DIM, "unchanged"),
            file.display()
        );
    } else {
        if !yes && !show_diff_and_confirm(&file, &diff)? {
            println!("{}", paint(DIM, "left as it was"));
            return Ok(());
        }
        write_whole(&file, &after)?;
        println!(
            "{} {added} hook events into {}",
            paint(render::GREEN, "installed"),
            file.display()
        );
    }
    println!("\n{} {trust}.", paint(BOLD, "One step left."));
    println!(
        "  {}",
        paint(
            DIM,
            "There is no roster for Codex, so nothing appears until a session does something."
        )
    );
    Ok(())
}

async fn disconnect_codex(yes: bool, json: bool) -> Result<()> {
    let file =
        crate::observe::codex::hooks_path().context("no home directory, so no ~/.codex to read")?;
    let before = read_existing(&file)?;
    let mut doc: serde_json::Map<String, serde_json::Value> = if before.trim().is_empty() {
        Default::default()
    } else {
        serde_json::from_str(&before).with_context(|| format!("{} is not JSON", file.display()))?
    };
    let removed = crate::observe::codex::uninstall(&mut doc);
    let after = if removed == 0 {
        before.clone()
    } else {
        serde_json::to_string_pretty(&doc)? + "\n"
    };
    let diff = crate::observe::connect::diff(&before, &after);
    if json {
        require_yes(yes, !diff.is_empty(), &file)?;
        if !diff.is_empty() {
            write_whole(&file, &after)?;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hooks_file": file.display().to_string(),
                "hooks_removed": removed,
                "diff": diff,
                "written": !diff.is_empty(),
            }))?
        );
        return Ok(());
    }
    if diff.is_empty() {
        println!(
            "{} nothing of ours in {}",
            paint(DIM, "unchanged"),
            file.display()
        );
        return Ok(());
    }
    if !yes && !show_diff_and_confirm(&file, &diff)? {
        println!("{}", paint(DIM, "left as it was"));
        return Ok(());
    }
    write_whole(&file, &after)?;
    println!(
        "{} {removed} hook entries from {}",
        paint(render::GREEN, "removed"),
        file.display()
    );
    Ok(())
}

/// Where telemetry and hooks send to: the host that is running, else the port
/// one takes by default.
fn host_base_url() -> String {
    config::read_host()
        .ok()
        .flatten()
        .map(|i| i.base_url())
        .unwrap_or_else(|| format!("http://127.0.0.1:{}", config::DEFAULT_PORT))
}

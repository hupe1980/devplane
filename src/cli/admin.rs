//! The installation itself: agents, search, the audit trail, diagnostics, connect.

use super::ConnectTarget;
use super::{raw, urlencode};
use crate::poller;
use crate::render::{BOLD, DIM, clip, paint};
use crate::{client, config, render};
use anyhow::{Context, Result};

pub async fn cmd_search(query: &str, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = raw(&c, &format!("/api/search?q={}", urlencode(query))).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = vec![];
    for hit in v.as_array().unwrap_or(&empty) {
        println!(
            "{}  {}",
            paint(DIM, &clip(hit["run_id"].as_str().unwrap_or(""), 12)),
            hit["text"].as_str().unwrap_or("")
        );
    }
    Ok(())
}

/// Prints what the inbox asked for and what became of it.
///
/// Three numbers per kind rather than one score, because they mean different
/// things: `acted` is the item doing its job, `dismissed` is somebody telling
/// you the kind is too loud, and `elsewhere` is ambiguous — the question was
/// answered in a terminal, so the item was right that a person was needed and
/// wrong about where they would be.
pub async fn cmd_audit(about: Option<&str>, limit: i64, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let path = match about {
        Some(id) => format!("/api/decisions?limit={limit}&about={}", urlencode(id)),
        None => format!("/api/decisions?limit={limit}"),
    };
    let v = raw(&c, &path).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = vec![];
    let rows = v.as_array().unwrap_or(&empty);
    if rows.is_empty() {
        println!("{}", paint(DIM, "Vibeplane has not decided anything yet."));
        return Ok(());
    }
    for d in rows {
        let actor = d["actor"].as_str().unwrap_or("");
        println!(
            "{} {:<7} {:<16} {}",
            paint(DIM, d["at"].as_str().unwrap_or("").get(0..19).unwrap_or("")),
            match (actor, d["outcome"].as_str().unwrap_or("")) {
                (_, "deny") | (_, "fail") => paint(render::RED, actor),
                ("human", _) => paint(render::BLUE, actor),
                _ => paint(DIM, actor),
            },
            d["action"].as_str().unwrap_or(""),
            clip(d["subject"].as_str().unwrap_or(""), 48)
        );
        // The reason is the whole point: "allowed" is not an answer, "allowed
        // by `Bash(pnpm test *)`" is.
        if let Some(r) = d["reason"].as_str() {
            println!("{:>21}{}", "", paint(DIM, &format!("↳ {}", clip(r, 70))));
        }
    }
    Ok(())
}

pub async fn cmd_agents(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v: serde_json::Value = c.get("/api/agents").await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = vec![];
    for a in v.as_array().unwrap_or(&empty) {
        println!(
            "{:<10} {:<14} {}",
            paint(BOLD, a["id"].as_str().unwrap_or("")),
            a["name"].as_str().unwrap_or(""),
            paint(DIM, a["command"].as_str().unwrap_or(""))
        );
    }
    println!(
        "\n{}\n{}",
        paint(
            DIM,
            "Add one by name in ~/.vibeplane/agents.toml:  [agents.kimi] command = \"...\""
        ),
        paint(
            DIM,
            "Or point at a command: vibeplane dispatch --agent '/opt/my-agent --acp' ..."
        )
    );
    Ok(())
}

/// Reads a project's configuration and reports on it.
///
/// Deliberately offline and daemon-free: the answer is a function of one file,
/// so it should be available in a repository you have not connected anything to
/// yet — and in CI, where the point is to fail the commit that broke it.
pub async fn cmd_diagnostics(json: bool) -> Result<()> {
    let settings_path = crate::observe::connect::settings_path()?;
    let settings = crate::observe::connect::read_settings(&settings_path).unwrap_or_default();
    let state = crate::observe::connect::inspect(&settings, &settings_path);

    let daemon = config::read_daemon_info()?;
    let diag = match client::Client::connect() {
        Ok(c) => raw(&c, "/api/diagnostics").await.ok(),
        Err(_) => None,
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "daemon": daemon,
                "connect": state,
                "diagnostics": diag,
            }))?
        );
        return Ok(());
    }

    println!("{}", paint(BOLD, "daemon"));
    match &daemon {
        Some(d) if poller::process_alive(d.pid) => println!(
            "  running   pid {} · port {} · v{}",
            d.pid, d.port, d.version
        ),
        Some(d) => println!(
            "  {} (stale record, pid {} is gone)",
            paint(render::RED, "not running"),
            d.pid
        ),
        None => println!("  {}", paint(render::RED, "not running")),
    }

    println!("\n{}", paint(BOLD, "claude code"));
    println!("  settings  {}", state.settings_path.display());
    if state.hooks_installed.is_empty() {
        println!(
            "  hooks     {} — run `vibeplane connect claude`",
            paint(render::RED, "not installed")
        );
    } else {
        println!(
            "  hooks     {} ({} events)",
            paint(render::GREEN, "installed"),
            state.hooks_installed.len()
        );
    }
    if state.gate_is_stale {
        println!(
            "  gate      {} — run `vibeplane connect claude`",
            paint(render::RED, "out of date")
        );
        println!(
            "            Prohibitions are answered on two hooks now. Without the second,\n\
             \x20           `never_auto` and `always_ask` never reach a session in auto mode,\n\
             \x20           where Claude Code approves routine calls without ever prompting."
        );
    }
    if state.allowlist_blocks_us {
        println!(
            "  {}",
            paint(
                render::RED,
                "allowedHttpHookUrls does not permit loopback — hooks will not run"
            )
        );
    }
    match (&state.telemetry_endpoint, state.telemetry_is_ours) {
        (Some(e), true) => println!("  telemetry {} → {}", paint(render::GREEN, "configured"), e),
        (Some(e), false) => println!(
            "  telemetry {} → {} (cost and context unavailable)",
            paint(render::YELLOW, "external"),
            e
        ),
        (None, _) => println!("  telemetry {}", paint(render::RED, "not configured")),
    }

    if let Some(d) = diag {
        println!("\n{}", paint(BOLD, "channels"));
        let empty = vec![];
        let channels = d["channels"].as_array().unwrap_or(&empty);
        if channels.is_empty() {
            println!("  {}", paint(DIM, "nothing received yet"));
        }
        for ch in channels {
            println!(
                "  {:<14} {:>6} events · worst {:>6} µs · last {}",
                ch["channel"].as_str().unwrap_or("?"),
                ch["count"].as_u64().unwrap_or(0),
                ch["worst_micros"].as_u64().unwrap_or(0),
                ch["last_seen_at"]
                    .as_str()
                    .unwrap_or("")
                    .get(11..19)
                    .unwrap_or("")
            );
            // The column was recorded from the first day and printed on none of
            // them. A malformed hook payload, an OTLP record that would not
            // parse, a work row the store refused — each was written here and
            // then shown to nobody, which is the same as not having it.
            if let Some(err) = ch["last_error"].as_str().filter(|e| !e.is_empty()) {
                // With its date, because an error carrying no date reads as
                // current — and this one is kept after it is fixed.
                let when = ch["last_error_at"]
                    .as_str()
                    .and_then(|t| t.get(..19))
                    .unwrap_or("");
                println!(
                    "  {:<14} {} {}",
                    "",
                    paint(DIM, when),
                    paint(render::RED, &clip(err, 60))
                );
            }
        }

        // The other gate. Vibeplane's own prohibitions reach auto mode, and in
        // that mode the thing deciding is a classifier configured somewhere
        // else entirely — so a person supervising twenty agents should be able
        // to see both from one place. Read, never written.
        let am = &d["auto_mode"];
        println!(
            "\n{}",
            paint(BOLD, "auto mode (Claude Code's own classifier)")
        );
        match am.get("unavailable") {
            Some(why) => {
                let said = match why.as_str() {
                    Some("no_claude") => "no claude binary on this machine",
                    Some("not_supported") => {
                        "this Claude Code does not report it (needs v2.1.198 or later)"
                    }
                    _ => "it answered with something this build could not read",
                };
                println!("  {}", paint(DIM, said));
            }
            None if am["configured"].as_bool() == Some(false) => {
                println!(
                    "  {}",
                    paint(
                        render::YELLOW,
                        "nothing configured — it trusts only this repo and its remotes"
                    )
                );
                println!(
                    "  {}",
                    paint(
                        DIM,
                        "every other destination reads as exfiltration; `/auto-mode-setup` drafts the entries"
                    )
                );
            }
            None => {
                let n = |k: &str| am[k].as_i64().unwrap_or(0);
                println!(
                    "  {} trusted entries · {} allow · {} soft deny · {} hard deny",
                    n("environment"),
                    n("allow"),
                    n("soft_deny"),
                    n("hard_deny")
                );
            }
        }
        println!(
            "  {}",
            paint(
                DIM,
                "your deny and ask rules resolve before it; it cannot override them"
            )
        );

        // A project whose `vibeplane.toml` will not load keeps the rules that
        // were already cached — and after a restart there are none to keep, so
        // its `never_auto` list is silently gone. That has to be visible
        // somewhere, and this is where somebody looks when something is wrong.
        let broken = d["unreadable_configs"].as_array().unwrap_or(&empty);
        if !broken.is_empty() {
            println!("\n{}", paint(BOLD, "unreadable configuration"));
            for b in broken {
                println!(
                    "  {} {}",
                    paint(render::RED, b["project"].as_str().unwrap_or("?")),
                    clip(b["error"].as_str().unwrap_or(""), 80)
                );
            }
            println!(
                "  {}",
                paint(
                    DIM,
                    "its permission rules are not in force — run `vibeplane check` there"
                )
            );
        }

        // The same failure one layer down, and the same reason for saying it
        // out loud: a stored row this build cannot decode is simply missing
        // from the board, which is indistinguishable from never having had it.
        let rows = d["unreadable_rows"].as_array().unwrap_or(&empty);
        if !rows.is_empty() {
            println!("\n{}", paint(BOLD, "unreadable rows"));
            for r in rows.iter().take(10) {
                println!(
                    "  {} {}",
                    paint(render::RED, r["row"].as_str().unwrap_or("?")),
                    clip(r["error"].as_str().unwrap_or(""), 80)
                );
            }
            if rows.len() > 10 {
                println!(
                    "  {}",
                    paint(DIM, &format!("… and {} more", rows.len() - 10))
                );
            }
            println!(
                "  {}",
                paint(
                    DIM,
                    "the schema changed under them. Observations rebuild from the providers, \
                     so deleting the database is safe; a `work` row is the one worth reading \
                     first, because it names a branch and a worktree."
                )
            );
        }
    }
    Ok(())
}

pub async fn cmd_connect(what: ConnectTarget, statusline: bool, json: bool) -> Result<()> {
    if what == ConnectTarget::Copilot {
        return connect_copilot(json).await;
    }
    // Start the daemon first: the hooks we are about to install point at it,
    // and a port in the settings file that nothing answers is worse than no
    // hooks at all.
    let c = client::Client::connect_or_start().await?;
    let token = config::load_or_create_token()?;
    let path = crate::observe::connect::settings_path()?;
    let mut settings = crate::observe::connect::read_settings(&path)?;
    let exe = std::env::current_exe().context("finding the vibeplane binary")?;
    let mut report = crate::observe::connect::connect(&mut settings, c.base_url(), &token, &exe);
    if statusline {
        report.notes.push(crate::observe::connect::wrap_status_line(
            &mut settings,
            &exe,
        ));
    }
    crate::observe::connect::write_settings(&path, &settings)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!(
        "{} {} hook events into {}",
        paint(render::GREEN, "installed"),
        report.hooks_added,
        path.display()
    );
    match report.telemetry {
        crate::observe::connect::TelemetryStatus::Configured => {
            println!(
                "{} telemetry → {}",
                paint(render::GREEN, "configured"),
                c.base_url()
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
        paint(BOLD, "vibeplane ls")
    );
    Ok(())
}

/// `vibeplane connect copilot`.
///
/// **One file, and one thing it deliberately does not do.**
///
/// Copilot loads every `*.json` in `~/.copilot/hooks/`, so the whole
/// installation is a single file Vibeplane owns — written, read back and
/// removed without touching a line the user wrote. That is a better mechanism
/// than Claude Code's, where hooks live inside the user's own `settings.json`
/// and disconnecting means picking our entries back out of it.
///
/// **Telemetry is not switched on here, and cannot be.** Copilot reads its OTel
/// configuration from the environment (`COPILOT_OTEL_ENABLED`,
/// `OTEL_EXPORTER_OTLP_ENDPOINT`) or from *managed* settings, which belong to an
/// organisation rather than to this tool. Editing somebody's shell profile is
/// not a thing a supervisor should do quietly, and writing a managed policy is
/// the same refusal that keeps Vibeplane out of the auto-mode classifier's
/// configuration. So the two lines are printed and the person runs them.
async fn connect_copilot(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let token = config::load_or_create_token()?;
    let exe = std::env::current_exe().context("finding the vibeplane binary")?;
    let dir = crate::observe::copilot::hooks_dir()
        .context("no home directory, so no ~/.copilot to write to")?;
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let file = dir.join(crate::observe::copilot::HOOKS_FILE);
    let contents = crate::observe::copilot::hooks_file(c.base_url(), &token, &exe);
    std::fs::write(&file, serde_json::to_string_pretty(&contents)?)
        .with_context(|| format!("writing {}", file.display()))?;

    let env = [
        ("COPILOT_OTEL_ENABLED", "true".to_string()),
        (
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            format!("{}/vibeplane/otel", c.base_url()),
        ),
    ];
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hooks_file": file.display().to_string(),
                "telemetry": "manual",
                "env": env.iter().map(|(k, v)| (k.to_string(), v.clone()))
                    .collect::<std::collections::BTreeMap<_, _>>(),
            }))?
        );
        return Ok(());
    }
    println!("{} {}", paint(render::GREEN, "wrote"), file.display());
    println!(
        "  {}",
        paint(
            DIM,
            "the permission gate runs as a command hook, because an HTTP one fails open there"
        )
    );
    println!(
        "
{} Copilot reads telemetry from the environment, so these two lines are yours to add:
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
            "Vibeplane does not edit shell profiles, and its managed-settings equivalent \
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

pub async fn cmd_disconnect(what: ConnectTarget, json: bool) -> Result<()> {
    if what == ConnectTarget::Copilot {
        let removed = crate::observe::copilot::hooks_dir()
            .map(|d| d.join(crate::observe::copilot::HOOKS_FILE))
            .filter(|f| f.exists())
            .inspect(|f| {
                std::fs::remove_file(f).ok();
            });
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "removed": removed.as_ref().map(|f| f.display().to_string()),
                }))?
            );
        } else {
            match removed {
                Some(f) => println!("{} {}", paint(render::GREEN, "removed"), f.display()),
                None => println!("{}", paint(DIM, "nothing of ours was installed")),
            }
            println!(
                "  {}",
                paint(DIM, "any OTel variables you exported are yours to remove")
            );
        }
        return Ok(());
    }

    let path = crate::observe::connect::settings_path()?;
    let mut settings = crate::observe::connect::read_settings(&path)?;
    let report = crate::observe::connect::disconnect(&mut settings);
    crate::observe::connect::write_settings(&path, &settings)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{} {} hook entries from {}",
            paint(render::GREEN, "removed"),
            report.hooks_removed,
            path.display()
        );
    }
    Ok(())
}

//! `devplane doctor`: channel health, the host, the app, the gate and each
//! GitHub host's sign-in, read from here and from a host that answers.

use crate::render::{BOLD, DIM, clip, paint};
use crate::{client, config, render};
use anyhow::Result;

/// The `app` section of `doctor`: whether this binary has the window, the
/// shortcut (validated by the plugin's own parser where built in), and the port.
fn app_section() -> serde_json::Value {
    let (cfg, problems) = config::app_config().unwrap_or_default();
    #[cfg(feature = "app")]
    let valid = serde_json::json!(crate::app::shortcut::validate(&cfg.shortcut).is_ok());
    #[cfg(not(feature = "app"))]
    let valid = serde_json::Value::Null;
    serde_json::json!({
        "built": cfg!(feature = "app"),
        "shortcut": cfg.shortcut,
        "shortcut_valid": valid,
        "port": cfg.port,
        "problems": problems,
    })
}

/// The `gate` section of `doctor`, printed once whether or not a host answers.
///
/// `SYNTAX_MODELLED_ON` is a compile-time fact about this binary;
/// `sessions_reporting_a_version` is a runtime fact only the host knows, and is
/// absent rather than zero when no host answers. A host built from a different
/// release than this CLI is said explicitly.
fn gate_section(diag: Option<&crate::view::Diagnostics>) {
    println!("\n{}", paint(BOLD, "gate"));
    let mine = crate::core::policy::SYNTAX_MODELLED_ON;
    println!(
        "  rule syntax modelled on Claude Code {}",
        paint(BOLD, mine)
    );

    if let Some(theirs) = diag.map(|d| d.gate.syntax_modelled_on.as_str())
        && theirs != mine
    {
        println!(
            "  {}",
            paint(
                render::YELLOW,
                &format!(
                    "the running host was built against {theirs} — it is the one deciding; \
                     restart it with `devplane down` to use this binary"
                )
            )
        );
    }

    println!(
        "  {}",
        paint(
            DIM,
            "prohibitions are Devplane's own and need no agreement from the agent"
        )
    );

    match diag.map(|d| d.gate.sessions_reporting_a_version) {
        None => println!("  {}", paint(DIM, "no host, so no session was asked")),
        Some(0) => println!(
            "  {}",
            paint(
                DIM,
                "no session reports its version — install the status-line shim to find out"
            )
        ),
        Some(n) => println!(
            "  {}",
            paint(DIM, &format!("{n} session(s) report a version"))
        ),
    }

    // Clock-reading coverage: `CLAUDE_AFK_TIMEOUT_MS` is readable only by the
    // `SessionStart` hook, so sessions started before `devplane agent connect` have
    // no reading, which is not the same as "nothing set".
    if let Some(d) = diag {
        let (read, unread) = (d.modes.clock_read, d.modes.clock_unread);
        if read + unread > 0 {
            println!(
                "  {}",
                paint(
                    DIM,
                    &format!(
                        "{read} of {} live session(s) had their environment read for \
                         `{}`; {unread} started before Devplane could",
                        read + unread,
                        crate::core::clock::ENV_KEY
                    )
                )
            );
        }
    }
}

/// The `started from …` line: only when the running host is a different
/// binary from this one, and silent when either side is unknown.
fn started_from(host_exe: Option<&str>, mine: Option<&str>) -> Option<String> {
    let (theirs, mine) = (host_exe?, mine?);
    (theirs != mine).then(|| format!("started from {theirs}"))
}

pub async fn cmd_doctor(json: bool) -> Result<()> {
    let settings_path = crate::observe::connect::settings_path()?;
    let settings = crate::observe::connect::read_settings(&settings_path).unwrap_or_default();
    let state = crate::observe::connect::inspect(&settings, &settings_path);
    // Run the gate, not just look for it in the settings file.
    let probe = crate::observe::connect::probe_gate(&settings);
    // Decisions the gate took while no host was listening: enforced, but not
    // yet in the audit trail until a host starts.
    let spooled = config::drain_spool_count();

    let host = config::read_host()?;
    // The record says where a host would be; whether one answers is asked.
    let (answers, diag) = match client::Client::connect() {
        Ok(c) if c.healthy().await => (true, c.diagnostics().await.ok()),
        _ => (false, None),
    };

    let provider = off_subscription(|k| std::env::var(k).ok());
    let app = app_section();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "host": host,
                "host_answers": answers,
                "app": app,
                "connect": state,
                "gate": probe,
                "measurement": {
                    "syntax_modelled_on": crate::core::policy::SYNTAX_MODELLED_ON,
                },
                "watched": {
                    "oldest_check": crate::core::vendors::CHECKED,
                    "rows": crate::core::vendors::watched().iter().map(|r| serde_json::json!({
                        "vendor": r.vendor,
                        "channel": r.channel.as_str(),
                        "reach": r.reach.as_str(),
                        "because": r.because,
                        "checked": r.checked,
                        "costs": r.channel.costs(),
                    })).collect::<Vec<_>>(),
                },
                "spooled_decisions": spooled,
                // Whether or not a host answers: the sign-in record says it.
                "github": super::github::doctor_hosts(diag.as_ref().map(|d| &d.forge.github)).0,
                "diagnostics": diag,
                "only_gate": provider,
            }))?
        );
        return Ok(());
    }

    println!("{}", paint(BOLD, "host"));
    match &host {
        Some(d) if answers => {
            println!("  running   port {} · v{}", d.port, d.version);
            // Said only when the running host is not this binary (e.g. an
            // `npx` copy left running after a proper install).
            let mine = std::env::current_exe()
                .ok()
                .map(|p| p.display().to_string());
            if let Some(line) = started_from(d.exe.as_deref(), mine.as_deref()) {
                println!("            {}", paint(DIM, &line));
            }
        }
        Some(d) => println!(
            "  {} (a record names port {}, but nothing answers there)",
            paint(render::RED, "not running"),
            d.port
        ),
        None => println!("  {}", paint(render::RED, "not running")),
    }

    println!("\n{}", paint(BOLD, "app"));
    if app["built"] == true {
        println!("  built     with the app feature — `devplane up --window` opens the window");
    } else {
        println!(
            "  built     {} — `cargo install devplane --features app` for the window",
            paint(DIM, "without the app feature")
        );
    }
    match app["problems"].as_array() {
        Some(ps) if !ps.is_empty() => {
            for p in ps {
                println!(
                    "  config    {} {}",
                    paint(render::RED, p["where"].as_str().unwrap_or("app.toml")),
                    p["what"].as_str().unwrap_or("")
                );
            }
        }
        _ => {}
    }
    let shortcut = app["shortcut"].as_str().unwrap_or("");
    match app["shortcut_valid"].as_bool() {
        Some(true) => println!("  shortcut  {shortcut}"),
        Some(false) => println!(
            "  shortcut  {} {}",
            paint(render::RED, shortcut),
            paint(
                DIM,
                "— not a shortcut the app can register; none is registered"
            )
        ),
        None => println!(
            "  shortcut  {shortcut} {}",
            paint(DIM, "(not checked: built without the app feature)")
        ),
    }
    println!(
        "  port      {}",
        match app["port"].as_u64() {
            Some(0) | None => "0 — the app picks a free one".to_string(),
            Some(p) => p.to_string(),
        }
    );

    // What is watched per vendor. Driving is not a column: every protocol
    // agent is driven identically.
    println!("\n{}", paint(BOLD, "watched"));
    println!(
        "  {}",
        paint(
            DIM,
            "a session you started yourself. Anything Devplane starts is driven over the \
             protocol and reports in full. Each row carries the date it was last read \
             against the vendor's own documentation."
        )
    );
    let rows = crate::core::vendors::watched();
    // Column widths are measured from the data, not constants.
    let width = rows.iter().map(|r| r.vendor.len()).max().unwrap_or(0);
    let chan_width = rows
        .iter()
        .map(|r| r.channel.as_str().len())
        .max()
        .unwrap_or(0);
    let reach_width = rows
        .iter()
        .map(|r| r.reach.as_str().len())
        .max()
        .unwrap_or(0);
    for vendor in crate::core::vendors::vendors() {
        for r in rows.iter().filter(|r| r.vendor == vendor) {
            use crate::core::vendors::Reach;
            // The word carries the distinction; colour only helps, so several
            // reach states may share one.
            let colour = match r.reach {
                Reach::Read => render::GREEN,
                Reach::Unproved => render::YELLOW,
                Reach::Unbuilt | Reach::NotPublished | Reach::Unchecked => DIM,
            };
            // `pad` guarantees at least one space, so the gap is part of each
            // column.
            println!(
                "  {}{}{}{}",
                render::pad(r.vendor, width + 2),
                render::pad(r.channel.as_str(), chan_width + 2),
                render::pad(&paint(colour, r.reach.as_str()), reach_width + 2),
                paint(DIM, &format!("{} · read {}", r.because, r.checked))
            );
        }
    }

    if let Some(because) = provider {
        println!(
            "\n  {} {}",
            paint(BOLD, "Devplane is the only gate on this machine."),
            paint(DIM, &format!("({because})"))
        );
    }

    // The rule syntax release this binary and the running host were built
    // against; they can differ when the host is an older binary.
    gate_section(diag.as_ref());

    println!("\n{}", paint(BOLD, "claude code"));
    println!("  settings  {}", state.settings_path.display());
    if state.hooks_installed.is_empty() {
        println!(
            "  hooks     {} — run `devplane agent connect claude`",
            paint(render::RED, "not installed")
        );
    } else {
        println!(
            "  hooks     {} ({} events)",
            paint(render::GREEN, "installed"),
            state.hooks_installed.len()
        );
    }
    match (&probe.command, probe.answered) {
        (None, _) => println!(
            "  gate      {} — run `devplane agent connect claude`",
            paint(render::RED, "not installed")
        ),
        (Some(_), true) => println!(
            "  gate      {} {}",
            paint(render::GREEN, "answering"),
            paint(DIM, &format!("({} ms, measured just now)", probe.millis))
        ),
        (Some(cmd), false) => {
            println!(
                "  gate      {} — every prohibition on this machine is inert",
                paint(render::RED, "INSTALLED AND NOT ANSWERING")
            );
            println!("            {}", paint(DIM, cmd));
            if let Some(e) = &probe.error {
                println!("            {}", paint(DIM, e));
            }
            println!(
                "            {}",
                paint(
                    DIM,
                    "a hook that does not answer never blocks — the provider carries on"
                )
            );
        }
    }
    if spooled > 0 {
        println!(
            "  pending   {} {}",
            paint(
                render::YELLOW,
                &format!("{spooled} decision(s) taken while no host was running")
            ),
            paint(DIM, "— they are enforced; start the host to file them")
        );
    }
    // A hook whose binary is gone runs nothing: that event is ungated,
    // whatever else the settings say.
    for (event, bin) in &state.gate_off {
        println!(
            "  gate      {} for {event} — its hook runs {bin}, which no longer exists",
            paint(render::RED, "off"),
        );
    }
    if !state.gate_off.is_empty() {
        println!(
            "            {}",
            paint(
                DIM,
                "run `devplane agent connect claude` from an installed devplane (or pass `--bin <path>`)"
            )
        );
    }
    if state.gate_is_stale {
        println!(
            "  gate      {} — run `devplane agent connect claude`",
            paint(render::RED, "out of date")
        );
        println!(
            "            The installed gate is not the one this build writes: both deciding\n\
             \x20           hooks must run this binary, and the holding one must outlast the\n\
             \x20           longest hold a project may declare."
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

    // GitHub: its own section, per host, whether or not a host runs.
    super::github::print_doctor(diag.as_ref().map(|d| &d.forge.github));
    if let Some(d) = diag {
        // Each ruled-out project with its reason, not just a count.
        for p in &d.forge.skipped_projects {
            println!(
                "  {}  {}  {}",
                paint(DIM, "not a GitHub repository"),
                p.project,
                paint(DIM, &clip(&p.reason, 60))
            );
        }

        println!("\n{}", paint(BOLD, "channels"));
        // The OpenCode subscription first: its feed does not replay, so a
        // dead subscription looks like a quiet machine from the counts alone.
        if let Some(oc) = &d.opencode {
            println!(
                "  {:<14} {}",
                "opencode",
                paint(
                    match oc.live {
                        true => render::GREEN,
                        false => render::RED,
                    },
                    &oc.says
                )
            );
        }
        if d.channels.is_empty() {
            println!("  {}", paint(DIM, "nothing received yet"));
        }
        for ch in &d.channels {
            println!(
                "  {:<14} {:>6} events · worst {:>6} µs · last {}",
                ch.channel,
                ch.count,
                ch.worst_micros,
                ch.last_seen_at.get(11..19).unwrap_or("")
            );
            // The last error on this channel: malformed hook payloads, OTLP
            // records that would not parse, rows the store refused.
            if let Some(err) = ch.last_error.as_deref().filter(|e| !e.is_empty()) {
                // Dated, because it is kept after the cause is fixed.
                let when = ch
                    .last_error_at
                    .as_deref()
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

        if let Some(attention) = d.attention.as_ref().filter(|a| a.is_object()) {
            super::inbox::print_attention(attention);
        }

        // Claude Code's auto-mode classifier, shown beside Devplane's own
        // rules. Read, never written.
        println!(
            "\n{}",
            paint(BOLD, "auto mode (Claude Code's own classifier)")
        );
        use crate::observe::automode::Unavailable;
        match &d.auto_mode {
            crate::view::AutoModeView::Unavailable { unavailable } => {
                let said = match unavailable {
                    Unavailable::NoClaude => "no claude binary on this machine",
                    Unavailable::NotSupported => {
                        "this Claude Code does not report it (needs v2.1.198 or later)"
                    }
                    Unavailable::Unreadable(_) => {
                        "it answered with something this build could not read"
                    }
                };
                println!("  {}", paint(DIM, said));
            }
            crate::view::AutoModeView::Read {
                configured: false, ..
            } => {
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
            crate::view::AutoModeView::Read {
                environment,
                allow,
                soft_deny,
                hard_deny,
                ..
            } => println!(
                "  {environment} trusted entries · {allow} allow · {soft_deny} soft deny · \
                 {hard_deny} hard deny"
            ),
        }
        println!(
            "  {}",
            paint(
                DIM,
                "your deny and ask rules resolve before it; it cannot override them"
            )
        );

        // A project whose `devplane.toml` will not load has no rules in force
        // after a restart; this is where that becomes visible.
        if !d.unreadable_configs.is_empty() {
            println!("\n{}", paint(BOLD, "unreadable configuration"));
            for b in &d.unreadable_configs {
                println!(
                    "  {} {}",
                    paint(render::RED, &b.project),
                    clip(&b.error, 80)
                );
            }
            println!(
                "  {}",
                paint(
                    DIM,
                    "its permission rules are not in force — run `devplane config check` there"
                )
            );
        }

        // Writes the store refused: logged and dropped rather than failing a
        // blocking hook, so this is the only place the gap is visible.
        let u = &d.unwritten;
        if u.events > 0 || u.decisions > 0 {
            println!("\n{}", paint(BOLD, "unwritten"));
            println!(
                "  {}",
                paint(
                    render::RED,
                    &format!(
                        "{} event(s) and {} decision(s) the store would not take",
                        u.events, u.decisions
                    )
                )
            );
            if let Some(why) = &u.last_reason {
                println!("  {}", clip(why, 80));
            }
            println!(
                "  {}",
                paint(
                    DIM,
                    "the board looks complete and is not. Check the disk and the \
                     permissions on the database, then restart — the gap does not \
                     fill in afterwards."
                )
            );
        }

        // Agents a previous host started that are still running, unreachable,
        // and costing money.
        if !d.leaked_agents.is_empty() {
            println!("\n{}", paint(BOLD, "leaked agents"));
            println!(
                "  {}",
                paint(
                    render::RED,
                    &format!(
                        "{} agent(s) started by an earlier host are still running and \
                         cannot be reached",
                        d.leaked_agents.len()
                    )
                )
            );
            for a in d.leaked_agents.iter().take(10) {
                println!(
                    "  {:<8}{}",
                    paint(BOLD, &a.pid.to_string()),
                    clip(&a.command, 64)
                );
                if let Some(w) = &a.worktree {
                    println!("  {:<8}{}", "", paint(DIM, &format!("holding {w}")));
                }
                println!(
                    "  {:<8}{}",
                    "",
                    paint(DIM, &format!("kill -TERM -{}", a.pid))
                );
            }
            println!(
                "  {}",
                paint(
                    DIM,
                    "Devplane does not end these for you: one may be part-way through \
                     writing what it was last asked to do."
                )
            );
        }

        // Stored rows this build cannot decode are otherwise simply missing.
        let rows = &d.unreadable_rows;
        if !rows.is_empty() {
            println!("\n{}", paint(BOLD, "unreadable rows"));
            for r in rows.iter().take(10) {
                println!("  {} {}", paint(render::RED, &r.row), clip(&r.error, 80));
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
                     so deleting the database is safe; a `change` row is the one worth reading \
                     first, because it names a branch and a worktree."
                )
            );
        }
    }
    Ok(())
}

/// The variable that moved this machine off a claude.ai sign-in — Bedrock,
/// Vertex, Foundry, a gateway or an API key — where the vendor's supervision
/// surfaces are off and Devplane is the only gate. `None` on a subscription.
fn off_subscription(get: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    let set = |k: &str| {
        get(k).is_some_and(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"))
    };
    for k in [
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
    ] {
        if set(k) {
            return Some(k);
        }
    }
    if get("ANTHROPIC_BASE_URL").is_some_and(|u| !u.is_empty() && !u.contains("api.anthropic.com"))
    {
        return Some("ANTHROPIC_BASE_URL");
    }
    ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"]
        .into_iter()
        .find(|k| set(k))
}

#[cfg(test)]
mod tests {
    use super::started_from;

    /// Speaks only when the binaries differ.
    #[test]
    fn the_started_from_line_speaks_only_when_the_binaries_differ() {
        assert_eq!(
            started_from(Some("/a/devplane"), Some("/a/devplane")),
            None,
            "one binary and one host needs no line at all"
        );
        assert_eq!(
            started_from(Some("/npx/devplane"), Some("/usr/local/bin/devplane")),
            Some("started from /npx/devplane".into()),
            "two copies and one host is the whole reason this exists, and it names the \
             host's binary rather than the one that was typed"
        );
    }

    /// Unknown is not different: either side unknown says nothing.
    #[test]
    fn an_unknown_binary_is_never_reported_as_a_different_one() {
        assert_eq!(started_from(None, Some("/usr/local/bin/devplane")), None);
        assert_eq!(started_from(Some("/npx/devplane"), None), None);
        assert_eq!(started_from(None, None), None);
    }
}

//! The installation itself: agents, search, the audit trail, diagnostics, connect.

use super::ConnectTarget;
use super::{raw, urlencode};
use crate::render::{BOLD, DIM, clip, paint};
use crate::{client, config, render};
use anyhow::{Context, Result};

pub async fn cmd_search(query: &str, json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let v = raw(&c, &format!("/api/search?q={}", urlencode(query))).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = vec![];
    let hits = v.as_array().unwrap_or(&empty);
    if hits.is_empty() {
        println!("No matches for `{query}`.");
        return Ok(());
    }
    for hit in hits {
        println!(
            "{}  {}",
            paint(DIM, &clip(hit["run_id"].as_str().unwrap_or(""), 12)),
            hit["text"].as_str().unwrap_or("")
        );
    }
    Ok(())
}

/// Prints the audit trail of decisions, or with `--otel` the same rows in the
/// GenAI semantic-conventions shape.
pub async fn cmd_audit(
    about: Option<&str>,
    without_me: bool,
    limit: i64,
    json: bool,
    otel: bool,
) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let filter = match without_me {
        true => "&without_me=true",
        false => "",
    };
    let path = match about {
        Some(id) => format!(
            "/api/decisions?limit={limit}{filter}&about={}",
            urlencode(id)
        ),
        None => format!("/api/decisions?limit={limit}{filter}"),
    };
    let v = raw(&c, &path).await?;

    // Rendered to stdout, not exported: Devplane receives OTLP and sends none.
    if otel {
        let rows: Vec<crate::core::Decision> =
            serde_json::from_value(v.clone()).unwrap_or_default();
        println!(
            "{}",
            serde_json::to_string_pretty(&crate::core::genai::payload(&rows))?
        );
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = vec![];
    let rows = v.as_array().unwrap_or(&empty);
    if rows.is_empty() {
        println!(
            "{}",
            paint(
                DIM,
                match without_me {
                    // Two different facts, so two different sentences.
                    true => "Nothing was decided in your name.",
                    false => "Devplane has not decided anything yet.",
                }
            )
        );
        return Ok(());
    }
    for d in rows {
        let authority = d["authority"].as_str().unwrap_or("");
        println!(
            "{} {} {} {}",
            paint(DIM, d["at"].as_str().unwrap_or("").get(0..19).unwrap_or("")),
            // `nobody` is coloured as a refusal whatever its outcome: the
            // moment passed without a person deciding.
            render::pad(
                &match (authority, d["outcome"].as_str().unwrap_or("")) {
                    ("nobody", _) => paint(render::RED, authority),
                    (_, "deny") | (_, "fail") => paint(render::RED, authority),
                    ("person", _) => paint(render::BLUE, authority),
                    ("timer", _) => paint(render::YELLOW, authority),
                    _ => paint(DIM, authority),
                },
                7
            ),
            render::pad(d["action"].as_str().unwrap_or(""), 16),
            clip(d["subject"].as_str().unwrap_or(""), 48)
        );
        if let Some(r) = d["reason"].as_str() {
            println!("{:>21}{}", "", paint(DIM, &format!("↳ {}", clip(r, 70))));
        }
        // Where an MCP tool was defined, reported and never graded. Three
        // states: a source, an MCP call whose source was not reported (said
        // explicitly), and a non-MCP call (nothing printed).
        let tool = d["tool"].as_str().unwrap_or("");
        match (
            d["server_source"].as_str(),
            crate::core::decision::could_carry_provenance(tool),
        ) {
            (Some(src), _) => {
                let named = match tool.is_empty() {
                    true => "that server",
                    false => tool,
                };
                println!(
                    "{:>21}{}",
                    "",
                    paint(DIM, &format!("↳ {named} was defined by: {src}"))
                );
            }
            (None, true) => println!(
                "{:>21}{}",
                "",
                paint(
                    DIM,
                    &format!("↳ {tool}: where it was defined was not reported here")
                )
            ),
            // Not an MCP tool: no source to have.
            (None, false) => {}
        }
    }
    Ok(())
}

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
fn gate_section(diag: &Option<serde_json::Value>) {
    println!("\n{}", paint(BOLD, "gate"));
    let mine = crate::core::policy::SYNTAX_MODELLED_ON;
    println!(
        "  rule syntax modelled on Claude Code {}",
        paint(BOLD, mine)
    );

    if let Some(theirs) = diag
        .as_ref()
        .and_then(|d| d["gate"]["syntax_modelled_on"].as_str())
        && theirs != mine
    {
        println!(
            "  {}",
            paint(
                render::YELLOW,
                &format!(
                    "the running host was built against {theirs} — it is the one deciding; \
                     restart it with `devplane quit` to use this binary"
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

    match diag.as_ref().map(|d| {
        d["gate"]["sessions_reporting_a_version"]
            .as_i64()
            .unwrap_or(0)
    }) {
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
    // `SessionStart` hook, so sessions started before `devplane connect` have
    // no reading, which is not the same as "nothing set".
    if let Some(d) = diag.as_ref() {
        let read = d["modes"]["clock_read"].as_i64().unwrap_or(0);
        let unread = d["modes"]["clock_unread"].as_i64().unwrap_or(0);
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

pub async fn cmd_diagnostics(json: bool) -> Result<()> {
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
        Ok(c) if c.healthy().await => (
            true,
            raw(&crate::local::Reader::Host(c), "/api/diagnostics")
                .await
                .ok(),
        ),
        _ => (false, None),
    };

    let provider = crate::core::provider::from_env();
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
                "github": super::github::doctor_hosts(diag.as_ref()).0,
                "diagnostics": diag,
                "provider": {
                    "name": provider.as_str(),
                    "because": provider.because(),
                    "vendor_supervision": provider.has_vendor_supervision(),
                    "off": provider.missing(),
                    "partial": provider.partial(),
                    "intact": provider.intact(),
                },
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
        println!("  built     with the app feature — `devplane app` opens the window");
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

    println!("\n{}", paint(BOLD, "provider"));
    println!(
        "  {}  {}",
        paint(
            if provider.has_vendor_supervision() {
                render::GREEN
            } else {
                render::YELLOW
            },
            provider.as_str()
        ),
        paint(DIM, &format!("({})", provider.because()))
    );
    if !provider.has_vendor_supervision() {
        println!(
            "  {}",
            paint(BOLD, "Devplane is the only gate on this machine.")
        );
        println!(
            "  {} {}",
            paint(DIM, "off here  "),
            paint(DIM, &provider.missing().join(" · "))
        );
        for row in provider.partial() {
            println!("  {} {}", paint(DIM, "partial   "), paint(DIM, row));
        }
        println!(
            "  {} {}",
            paint(DIM, "still on  "),
            paint(DIM, &provider.intact().join(" · "))
        );
    }

    // The rule syntax release this binary and the running host were built
    // against; they can differ when the host is an older binary.
    gate_section(&diag);

    println!("\n{}", paint(BOLD, "claude code"));
    println!("  settings  {}", state.settings_path.display());
    if state.hooks_installed.is_empty() {
        println!(
            "  hooks     {} — run `devplane connect claude`",
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
            "  gate      {} — run `devplane connect claude`",
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
                "run `devplane connect claude` from an installed devplane (or pass `--bin <path>`)"
            )
        );
    }
    if state.gate_is_stale {
        println!(
            "  gate      {} — run `devplane connect claude`",
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
    super::github::print_doctor(diag.as_ref());
    if let Some(d) = diag {
        // Each ruled-out project with its reason, not just a count.
        let f = &d["forge"];
        for p in f["skipped_projects"].as_array().unwrap_or(&vec![]) {
            println!(
                "  {}  {}  {}",
                paint(DIM, "not a GitHub repository"),
                p["project"].as_str().unwrap_or(""),
                paint(DIM, &clip(p["reason"].as_str().unwrap_or(""), 60))
            );
        }

        println!("\n{}", paint(BOLD, "channels"));
        // The OpenCode subscription first: its feed does not replay, so a
        // dead subscription looks like a quiet machine from the counts alone.
        if let Some(oc) = d["opencode"].as_object() {
            let live = oc.get("live").and_then(|v| v.as_bool()).unwrap_or(false);
            println!(
                "  {:<14} {}",
                "opencode",
                match live {
                    true => paint(render::GREEN, oc["says"].as_str().unwrap_or("")),
                    false => paint(render::RED, oc["says"].as_str().unwrap_or("")),
                }
            );
        }
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
            // The last error on this channel: malformed hook payloads, OTLP
            // records that would not parse, rows the store refused.
            if let Some(err) = ch["last_error"].as_str().filter(|e| !e.is_empty()) {
                // Dated, because it is kept after the cause is fixed.
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

        // Claude Code's auto-mode classifier, shown beside Devplane's own
        // rules. Read, never written.
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

        // A project whose `devplane.toml` will not load has no rules in force
        // after a restart; this is where that becomes visible.
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
                    "its permission rules are not in force — run `devplane check` there"
                )
            );
        }

        // Writes the store refused: logged and dropped rather than failing a
        // blocking hook, so this is the only place the gap is visible.
        let u = &d["unwritten"];
        let (ue, ud) = (
            u["events"].as_u64().unwrap_or(0),
            u["decisions"].as_u64().unwrap_or(0),
        );
        if ue > 0 || ud > 0 {
            println!("\n{}", paint(BOLD, "unwritten"));
            println!(
                "  {}",
                paint(
                    render::RED,
                    &format!("{ue} event(s) and {ud} decision(s) the store would not take")
                )
            );
            if let Some(why) = u["last_reason"].as_str() {
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
        let leaked = d["leaked_agents"].as_array().unwrap_or(&empty);
        if !leaked.is_empty() {
            println!("\n{}", paint(BOLD, "leaked agents"));
            println!(
                "  {}",
                paint(
                    render::RED,
                    &format!(
                        "{} agent(s) started by an earlier host are still running and \
                         cannot be reached",
                        leaked.len()
                    )
                )
            );
            for a in leaked.iter().take(10) {
                let pid = a["pid"].as_u64().unwrap_or(0);
                println!(
                    "  {:<8}{}",
                    paint(BOLD, &pid.to_string()),
                    clip(a["command"].as_str().unwrap_or(""), 64)
                );
                if let Some(w) = a["worktree"].as_str() {
                    println!("  {:<8}{}", "", paint(DIM, &format!("holding {w}")));
                }
                println!("  {:<8}{}", "", paint(DIM, &format!("kill -TERM -{pid}")));
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
                     so deleting the database is safe; a `change` row is the one worth reading \
                     first, because it names a branch and a worktree."
                )
            );
        }
    }
    Ok(())
}

pub async fn cmd_connect(
    what: ConnectTarget,
    statusline: bool,
    yes: bool,
    json: bool,
    bin: Option<std::path::PathBuf>,
) -> Result<()> {
    // Every hook runs this path; one that vanishes turns the gate off.
    let exe = stable_binary(bin)?;
    if what == ConnectTarget::Copilot {
        return connect_copilot(yes, json, &exe).await;
    }
    if what == ConnectTarget::Codex {
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
        paint(BOLD, "devplane ls")
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

/// Why a binary path would not outlive the hooks that name it, if it would
/// not: a package runner's cache, a macOS App Translocation mount, a mounted
/// disk image, or an AppImage's mount.
#[must_use]
pub fn transient_binary(path: &std::path::Path) -> Option<&'static str> {
    let p = path.to_string_lossy().replace('\\', "/");
    if p.contains("/_npx/") || p.contains("/.npm/_npx/") || p.contains("/npm-cache/_npx/") {
        return Some("inside npx's package cache, which npm clears and replaces");
    }
    if p.contains("/AppTranslocation/") {
        return Some("a macOS App Translocation path, a read-only copy that moves on every launch");
    }
    if p.starts_with("/Volumes/") {
        return Some("on a mounted volume (a .dmg, say), which is gone once it is ejected");
    }
    if p.contains("/.mount_") {
        return Some("inside an AppImage's mount, which exists only while it runs");
    }
    None
}

/// The binary every hook will run: `--bin` when given, else this one — unless
/// this one lives somewhere that will disappear, which is refused with the
/// reason and the fix. An AppImage names itself through `$APPIMAGE`.
fn stable_binary(bin: Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(bin) = bin {
        let bin = std::fs::canonicalize(&bin)
            .with_context(|| format!("--bin {}: no such file", bin.display()))?;
        if !bin.is_file() {
            anyhow::bail!("--bin {}: not a file", bin.display());
        }
        return Ok(bin);
    }
    // `$APPIMAGE` is set by the AppImage runtime; trusted only when it names
    // the file, since any process can set a variable.
    if let Some(image) = std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
        let image = std::path::PathBuf::from(image);
        if image.is_file() {
            return Ok(image);
        }
        anyhow::bail!(
            "not connecting: $APPIMAGE names {}, which is not a file; pass --bin <path>",
            image.display()
        );
    }
    let exe = std::env::current_exe().context("finding the devplane binary")?;
    match transient_binary(&exe) {
        None => Ok(exe),
        Some(why) => anyhow::bail!(
            "not connecting: every hook runs the binary's path, and {} is {why}, so the hooks \
             — and the gate — would stop working without a word when it goes. Install \
             devplane (`npm install -g devplane`, `cargo install devplane`, or copy the \
             binary somewhere permanent) and run `connect` from there, or pass \
             `--bin <path>` naming a devplane binary that stays put.",
            exe.display()
        ),
    }
}

/// `devplane connect copilot`.
///
/// Copilot loads every `*.json` in `~/.copilot/hooks/`, so the installation is
/// one file Devplane owns, and every hook in it runs this binary. Telemetry
/// cannot be switched on from here (it lives in the environment or managed
/// settings), so the environment lines are printed for the person to run.
async fn connect_copilot(yes: bool, json: bool, exe: &std::path::Path) -> Result<()> {
    let base_url = host_base_url();
    let token = config::ingest_token(&config::load_or_create_token()?);
    let dir = crate::observe::copilot::hooks_dir()
        .context("no home directory, so no ~/.copilot to write to")?;
    let file = dir.join(crate::observe::copilot::HOOKS_FILE);
    let contents = crate::observe::copilot::hooks_file(exe);
    let after = serde_json::to_string_pretty(&contents)? + "\n";
    // The file is entirely ours: diff against what we wrote last time, if any.
    let before = read_existing(&file)?;
    let diff = crate::observe::connect::diff(&before, &after);

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
            std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
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
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
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

pub async fn cmd_disconnect(what: ConnectTarget, yes: bool, json: bool) -> Result<()> {
    if what == ConnectTarget::Codex {
        return disconnect_codex(yes, json).await;
    }
    if what == ConnectTarget::Copilot {
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

/// `devplane connect codex`: Devplane's entries merged into `~/.codex/hooks.json`.
///
/// The file is the person's, so entries are merged in and picked back out on
/// disconnect. No telemetry (none is documented). Codex silently skips any
/// hook the person has not approved in its own dialog, so that is said last.
async fn connect_codex(yes: bool, json: bool, exe: &std::path::Path) -> Result<()> {
    let file = crate::observe::codex::hooks_path()
        .context("no home directory, so no ~/.codex to write to")?;
    let before = read_existing(&file)?;
    let mut doc: serde_json::Map<String, serde_json::Value> = if before.trim().is_empty() {
        Default::default()
    } else {
        serde_json::from_str(&before).with_context(|| format!("{} is not JSON", file.display()))?
    };
    let added = crate::observe::codex::install(&mut doc, exe);
    let after = serde_json::to_string_pretty(&doc)? + "\n";
    let diff = crate::observe::connect::diff(&before, &after);
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

/// A file of the person's, or empty when there is none. Any other failure —
/// unreadable, not UTF-8 — is an error: read as empty, the file would be
/// overwritten with only Devplane's entries in it.
fn read_existing(file: &std::path::Path) -> Result<String> {
    match std::fs::read_to_string(file) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}; left as it was", file.display())),
    }
}

/// Written beside and renamed over, so an interrupted write leaves the
/// person's file whole.
fn write_whole(file: &std::path::Path, contents: &str) -> Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let tmp = file.with_extension(format!("devplane-{}.tmp", std::process::id()));
    std::fs::write(&tmp, contents).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, file)
        .inspect_err(|_| {
            std::fs::remove_file(&tmp).ok();
        })
        .with_context(|| format!("writing {}", file.display()))
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

/// `devplane rewind` — the files the vendor's checkpoint will not restore.
///
/// A query over existing rows, no snapshots: Claude Code checkpoints what its
/// own editing tools touch; this names what shell commands wrote past it.
pub async fn cmd_rewind(run: &str, json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    // The missing host is the cause when there is one; "not on the board"
    // would be a wrong guess on top of it.
    let v: serde_json::Value = c
        .get(&format!("/api/runs/{run}/rewind-gap"))
        .await
        .map_err(|e| match crate::client::is_no_host(&e) {
            true => e,
            false => e.context("that run is not on the board"),
        })?;
    let files: Vec<&str> = v["files"]
        .as_array()
        .map(|a| a.iter().filter_map(|f| f.as_str()).collect())
        .unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    if files.is_empty() {
        println!(
            "  {}",
            paint(
                DIM,
                "no shell command in this session named a file for writing — /rewind covers it"
            )
        );
        return Ok(());
    }
    println!(
        "{}",
        paint(BOLD, "outside Claude Code's checkpoint for this session")
    );
    for f in &files {
        println!("  {f}");
    }
    println!();
    println!(
        "  {}",
        paint(
            DIM,
            "a shell command named these for writing; /rewind restores only what"
        )
    );
    println!(
        "  {}",
        paint(DIM, "Claude Code's own editing tools touched")
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

    /// `connect` writes the binary's path into every hook, so a path that
    /// will vanish is refused with the reason, and a stable one is not.
    #[test]
    fn a_binary_that_will_disappear_is_named_as_such() {
        use std::path::Path;
        for (p, word) in [
            (
                "/Users/me/.npm/_npx/1a2b/node_modules/devplane/bin/devplane",
                "npx",
            ),
            (
                "/private/var/folders/x/T/AppTranslocation/ABC/d/Devplane.app/Contents/MacOS/devplane",
                "Translocation",
            ),
            (
                "/Volumes/Devplane/Devplane.app/Contents/MacOS/devplane",
                "mounted",
            ),
            ("/tmp/.mount_DevplaXYZ/usr/bin/devplane", "AppImage"),
        ] {
            let why = super::transient_binary(Path::new(p)).unwrap_or_else(|| panic!("{p}"));
            assert!(why.contains(word), "{p}: {why}");
        }
        for p in [
            "/usr/local/bin/devplane",
            "/opt/homebrew/bin/devplane",
            "/Users/me/.cargo/bin/devplane",
        ] {
            assert_eq!(super::transient_binary(Path::new(p)), None, "{p}");
        }
    }
}

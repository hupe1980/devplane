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
pub async fn cmd_audit(
    about: Option<&str>,
    without_me: bool,
    limit: i64,
    json: bool,
) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
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
                    // Two different facts, and a surface that prints one
                    // sentence for both is telling somebody the wrong one half
                    // the time.
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
            // `nobody` is coloured like a refusal whatever its outcome says,
            // because it is one: the agent asked, the moment passed, and the
            // thing that did not happen was a person deciding. It is the row
            // this log exists to be able to show, so it does not read as dim
            // background the way `daemon` does.
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
        // The reason is the whole point: "allowed" is not an answer, "allowed
        // by `Bash(pnpm test *)`" is.
        if let Some(r) = d["reason"].as_str() {
            println!("{:>21}{}", "", paint(DIM, &format!("↳ {}", clip(r, 70))));
        }
        // **Where the thing that acted came from**, on the rows that can have
        // one. Not a column: a tool that is not an MCP tool cannot have a
        // source, and a reserved blank would teach the reader that the blank
        // means something.
        //
        // Reported, never graded. Nothing here says one provenance is safer
        // than another — the person decides what `project` is worth in a
        // repository they have just cloned.
        //
        // **Three states, and the middle one used to be invisible.** A call
        // that cannot have a source prints nothing, and so did an MCP call
        // whose source nobody sent — so `mcp__linear__create` from a vendor
        // with no such field looked exactly like `Bash(ls)`, and a blank
        // taught the reader *ordinary tool* when it meant *unknowable here*.
        // The same distinction the capability table draws between *not probed*
        // and *not supported*.
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
            // Not an MCP tool. There is no source to have, and a reserved blank
            // would teach the reader that the blank means something.
            (None, false) => {}
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
    let mut any_measured = false;
    for a in v.as_array().unwrap_or(&empty) {
        println!(
            "{:<10} {:<14} {}",
            paint(BOLD, a["id"].as_str().unwrap_or("")),
            a["name"].as_str().unwrap_or(""),
            paint(DIM, a["command"].as_str().unwrap_or(""))
        );
        // **What this agent was measured to support, and only where it was.**
        // Every session capability is advertised per agent at `initialize`, so
        // it is a runtime fact about that agent at that version. An agent
        // Devplane has never started has no line here at all — *not probed* is
        // a different fact from *not supported*, and printing a row of crosses
        // about something nobody has asked would state the second while meaning
        // the first.
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
            "Or point at a command: devplane dispatch --agent '/opt/my-agent --acp' ..."
        )
    );
    Ok(())
}

/// The one `gate` section of `doctor`.
///
/// **One section, printed once, whether or not a daemon answers.** It used to
/// be two: one built from this binary's own constant and one from the running
/// daemon's `/api/diagnostics`, under the same heading, opening with the same
/// sentence. Neither was wrong and the pair was unreadable.
///
/// The two facts it carries are different in kind and are labelled as such.
/// `SYNTAX_MODELLED_ON` is a **compile-time** fact about the binary you are
/// holding; `sessions_reporting_a_version` is a **runtime** fact only the daemon
/// can know, and it is absent rather than zero when nothing is running — because
/// *no daemon has been asked* and *no session reports a version* are two
/// different states and rendering them identically is how a diagnostic starts
/// lying.
///
/// And where the daemon was built against a different release from this CLI,
/// that is **said**. A stale daemon is a real state — `devplane stop` is one
/// command and nothing restarts it automatically — and it is exactly the state
/// in which a person reading `doctor` would otherwise be told a number that is
/// not the one enforcing anything.
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
                    "the running daemon was built against {theirs} — it is the one deciding; \
                     restart it with `devplane stop` to use this binary"
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
        None => println!("  {}", paint(DIM, "no daemon, so no session was asked")),
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

    // **How much of the question-clock reading this machine actually has.**
    // `CLAUDE_AFK_TIMEOUT_MS` is readable only by the `SessionStart` hook, which
    // runs as a child of the session — so a session that started before
    // `devplane connect` has no reading at all, and `devplane modes` staying
    // silent about it would be saying *your questions wait for you* about
    // sessions it has never looked at. The coverage of a surface is a fact
    // about it, not something a reader should assume.
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

/// Reads a project's configuration and reports on it.
///
/// Deliberately offline and daemon-free: the answer is a function of one file,
/// so it should be available in a repository you have not connected anything to
/// yet — and in CI, where the point is to fail the commit that broke it.
pub async fn cmd_diagnostics(json: bool) -> Result<()> {
    let settings_path = crate::observe::connect::settings_path()?;
    let settings = crate::observe::connect::read_settings(&settings_path).unwrap_or_default();
    let state = crate::observe::connect::inspect(&settings, &settings_path);
    // Not "is a line in the settings file", which is what every previous
    // version of this check answered: **run it**. Every way this layer has been
    // wrong was a gate that read as installed and decided nothing.
    let probe = crate::observe::connect::probe_gate(&settings);
    // Decisions the gate took while no daemon was listening. They are enforced
    // and not yet written down, which is a state worth naming: the audit trail
    // is behind, and the only thing that catches it up is starting the daemon.
    let spooled = config::drain_spool_count();

    let daemon = config::read_daemon_info()?;
    let diag = match client::Client::connect() {
        Ok(c) => raw(&c, "/api/diagnostics").await.ok(),
        Err(_) => None,
    };

    let provider = crate::core::provider::from_env();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "daemon": daemon,
                "connect": state,
                "gate": probe,
                "measurement": {
                    "syntax_modelled_on": crate::core::policy::SYNTAX_MODELLED_ON,
                },
                "watched": {
                    "checked": crate::core::vendors::CHECKED,
                    "rows": crate::core::vendors::watched().iter().map(|r| serde_json::json!({
                        "vendor": r.vendor,
                        "channel": r.channel.as_str(),
                        "reach": r.reach.as_str(),
                        "because": r.because,
                        "costs": r.channel.costs(),
                    })).collect::<Vec<_>>(),
                },
                "spooled_decisions": spooled,
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

    // Which world this machine is in, before anything about channels — because
    // on four of the six providers the vendor's whole supervision layer is off
    // and Devplane is the only gate here, and a person needs telling that
    // before they are told a hook is installed.
    // **What is watched here, per vendor.** Driving is not in this table: every
    // agent that speaks the protocol is driven identically, so a column for it
    // would be five identical ticks and would invite averaging the two halves
    // into one impression — which is the misreading the table exists to stop.
    println!("\n{}", paint(BOLD, "watched"));
    println!(
        "  {}",
        paint(
            DIM,
            &format!(
                "a session you started yourself. Anything Devplane starts is driven over the \
                 protocol and reports in full. Checked {}",
                crate::core::vendors::CHECKED
            )
        )
    );
    let rows = crate::core::vendors::watched();
    let width = rows.iter().map(|r| r.vendor.len()).max().unwrap_or(0);
    for vendor in crate::core::vendors::vendors() {
        for r in rows.iter().filter(|r| r.vendor == vendor) {
            use crate::core::vendors::Reach;
            let colour = match r.reach {
                Reach::Read => render::GREEN,
                Reach::Unproved => render::YELLOW,
                Reach::NotPublished => DIM,
            };
            // **`pad` guarantees at least one space**, so a string exactly
            // `width` long comes back one column wider — which is right for a
            // table whose columns must not touch, and wrong if the caller then
            // adds its own separator. The gap is part of the column here.
            println!(
                "  {}{}{}{}",
                render::pad(r.vendor, width + 2),
                render::pad(r.channel.as_str(), 14),
                render::pad(&paint(colour, r.reach.as_str()), 16),
                paint(DIM, r.because)
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

    // The release this crate's rule syntax was read against — a fact with a
    // date, and nothing derived from it.
    //
    // A warning used to live here: how many releases a running session was
    // past that number, in yellow. It was deleted on 2026-09-19 because it
    // measured the decay of a claim this product stopped making. The floor
    // existed to protect *Claude Code would have approved this too*; nothing
    // answers yes on the vendor's behalf any more, so the only thing the count
    // still tracked was how long ago somebody wrote the number down.
    //
    // **This section is printed exactly once.** There were two of it until
    // 2026-09-20 — this one from the CLI's own constant, and a second one under
    // the same heading built from the daemon's `/api/diagnostics`, both opening
    // with the same sentence. A person running `doctor` saw `gate` twice and
    // had no way to tell which was which, in the command whose whole job is
    // saying what is true. The two were not redundant, which is the part worth
    // keeping: the daemon can be an **older binary than the CLI**, so the two
    // constants can genuinely disagree — and that is now said out loud instead
    // of being rendered as a repetition.
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
                &format!("{spooled} decision(s) taken while no daemon was running")
            ),
            paint(DIM, "— they are enforced; start the daemon to file them")
        );
    }
    if state.gate_is_stale {
        println!(
            "  gate      {} — run `devplane connect claude`",
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
        // GitHub, read for every registered project. Its own section because
        // the failure that matters — `gh` not logged in — is one no hook or
        // roster line would ever mention.
        println!("\n{}", paint(BOLD, "github"));
        let f = &d["forge"];
        match (f["viewer"].as_str(), f["error"].as_str()) {
            (_, Some(e)) if !e.is_empty() => {
                println!(
                    "  {}  {}",
                    paint(render::YELLOW, "not read"),
                    paint(DIM, &clip(e, 70))
                )
            }
            (Some(v), _) => println!(
                "  as {} · {} project(s) with a forge · {} ruled out · last read {}",
                v,
                f["projects"].as_u64().unwrap_or(0),
                f["skipped"].as_u64().unwrap_or(0),
                f["last_poll_at"]
                    .as_str()
                    .and_then(|t| t.get(11..19))
                    .unwrap_or("never")
            ),
            (None, _) => println!("  {}", paint(DIM, "not read yet")),
        }
        // "3 skipped" is a number a person can do nothing with. The reason is
        // the value: usually "no GitHub remote", and when it is not, this is
        // where a wrong guess about what is permanent becomes visible.
        for p in f["skipped_projects"].as_array().unwrap_or(&vec![]) {
            println!(
                "  {}  {}  {}",
                paint(DIM, "ruled out"),
                p["project"].as_str().unwrap_or(""),
                paint(DIM, &clip(p["reason"].as_str().unwrap_or(""), 60))
            );
        }

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

        // The other gate. Devplane's own prohibitions reach auto mode, and in
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

        // A project whose `devplane.toml` will not load keeps the rules that
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
                    "its permission rules are not in force — run `devplane check` there"
                )
            );
        }

        // **The third of the same failure, and the worst of them.** A write
        // the store refused is history that never arrived at all — there is no
        // row to fail to decode and no file to fix. It is logged and dropped
        // on purpose, because failing the hook a session is blocked on is the
        // worse trade; what was missing was anywhere for a person to see it.
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

        // **The failure that costs money while nobody is looking.** An agent a
        // previous daemon started, still running, unreachable — and the only
        // one of these absences whose price goes up the longer it is missed.
        let leaked = d["leaked_agents"].as_array().unwrap_or(&empty);
        if !leaked.is_empty() {
            println!("\n{}", paint(BOLD, "leaked agents"));
            println!(
                "  {}",
                paint(
                    render::RED,
                    &format!(
                        "{} agent(s) started by an earlier daemon are still running and \
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
    let exe = std::env::current_exe().context("finding the devplane binary")?;
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
        paint(BOLD, "devplane ls")
    );
    Ok(())
}

/// `devplane connect copilot`.
///
/// **One file, and one thing it deliberately does not do.**
///
/// Copilot loads every `*.json` in `~/.copilot/hooks/`, so the whole
/// installation is a single file Devplane owns — written, read back and
/// removed without touching a line the user wrote. That is a better mechanism
/// than Claude Code's, where hooks live inside the user's own `settings.json`
/// and disconnecting means picking our entries back out of it.
///
/// **Telemetry is not switched on here, and cannot be.** Copilot reads its OTel
/// configuration from the environment (`COPILOT_OTEL_ENABLED`,
/// `OTEL_EXPORTER_OTLP_ENDPOINT`) or from *managed* settings, which belong to an
/// organisation rather than to this tool. Editing somebody's shell profile is
/// not a thing a supervisor should do quietly, and writing a managed policy is
/// the same refusal that keeps Devplane out of the auto-mode classifier's
/// configuration. So the two lines are printed and the person runs them.
async fn connect_copilot(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let token = config::load_or_create_token()?;
    let exe = std::env::current_exe().context("finding the devplane binary")?;
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
            format!("{}/devplane/otel", c.base_url()),
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

/// `devplane rewind` — the files the vendor's checkpoint will not restore.
///
/// A query over rows that already exist, and deliberately not a feature: no
/// snapshots, no storage, no second copy of anybody's files. Claude Code
/// checkpoints what its own editing tools touch; this names what a shell
/// command wrote past it, which is the gap its own documentation states.
pub async fn cmd_rewind(run: &str, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v: serde_json::Value = c
        .get(&format!("/api/runs/{run}/rewind-gap"))
        .await
        .context("that run is not on the board")?;
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

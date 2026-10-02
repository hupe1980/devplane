//! Reading changes: the list, one change in full, and its done certificate.
//! Read as [`ChangeView`], the struct the host serialises, from a host that
//! answers or else the store.

use super::ids;
use crate::core::change::{ChangeState, Outcome, Standing, Stopped};
use crate::render::{self, BOLD, DIM, clip, paint};
use crate::view::ChangeView;
use anyhow::{Context, Result};

/// The colour a standing is painted in, one rule for the list and the detail.
fn standing_colour(standing: &Standing) -> &'static str {
    match standing {
        Standing::Verified => render::GREEN,
        Standing::Failed { .. } => render::RED,
        Standing::Stale { .. } | Standing::ChecksChanged { .. } => render::YELLOW,
        _ => DIM,
    }
}

pub(super) async fn cmd_list(json: bool) -> Result<()> {
    let items = crate::local::Reader::open().await?.changes().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }
    if items.is_empty() {
        println!(
            "No changes yet.\n\n  {}",
            paint(BOLD, "devplane change start \"fix the flaky login test\"")
        );
        return Ok(());
    }
    for w in &items {
        print_row(w);
    }
    Ok(())
}

/// One change as a list row: state, title, the gates' word, what it waits on,
/// its plan beside the verdict, and where it lives.
fn print_row(w: &ChangeView<'_>) {
    let gates = paint(standing_colour(&w.standing), &w.standing_says);
    let waiting = [w.waiting_says.as_deref(), w.in_place_says.as_deref()]
        .into_iter()
        .flatten()
        .map(|s| paint(render::YELLOW, s))
        .collect::<Vec<_>>()
        .join("  ");
    println!(
        "{} {:<40} {:<10} {}{}",
        match w.state {
            ChangeState::Verified => paint(render::GREEN, w.state.glyph()),
            ChangeState::InFlight => paint(render::BLUE, w.state.glyph()),
            ChangeState::Archived => paint(DIM, w.state.glyph()),
            _ => w.state.glyph().to_string(),
        },
        clip(&w.change.title, 40),
        w.state.as_str(),
        gates,
        match waiting.is_empty() {
            true => String::new(),
            false => format!("  {waiting}"),
        }
    );
    // The plan beside the verdict: "done, with eleven boxes unticked". The
    // host composes the judgement; this renders it.
    if let Some(plan) = &w.plan {
        let mut parts: Vec<String> = Vec::new();
        match (plan.present, plan.progress) {
            (false, _) => parts.push(paint(render::RED, "no such specification")),
            (true, Some(p)) if p.open() > 0 => parts.push(paint(
                render::YELLOW,
                &format!("{} of {} open", p.open(), p.total),
            )),
            (true, Some(p)) => parts.push(paint(DIM, &format!("all {} done", p.total))),
            // Absent is absent. Never `0 of 0`, never a full bar.
            (true, None) => parts.push(paint(DIM, "no task list")),
        }
        let q = plan.open_questions;
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
            paint(DIM, &plan.path),
            parts.join(paint(DIM, " · ").as_str())
        );
        if w.plan_contradicts_done {
            println!(
                "    {}",
                paint(render::RED, "done, and the plan it answers is not")
            );
        }
    }
    println!("  {}", paint(DIM, &w.shape_says));
    let cost = match w.change.cost_usd {
        c if c > 0.0 => format!("  ${c:.2}"),
        _ => String::new(),
    };
    let pr = match &w.change.pull_request {
        Some(p) => format!("  #{} {}", p.number, p.status),
        None => String::new(),
    };
    println!(
        "  {}",
        paint(
            DIM,
            &format!(
                "{}  {}{cost}{pr}",
                w.change.id.as_str(),
                w.change.branch.as_deref().unwrap_or("")
            )
        )
    );
}

pub(super) async fn cmd_show(change: &str, json: bool) -> Result<()> {
    let reader = crate::local::Reader::open().await?;
    let id = ids::change(&reader, change).await?;
    // The one change, whose tree the host reads now rather than from its
    // last tick.
    let w = reader.change(&id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&w)?);
        return Ok(());
    }
    print_change(&w);
    Ok(())
}

pub(super) async fn cmd_export(
    change: &str,
    json: bool,
    out: Option<&std::path::Path>,
    force: bool,
    sign: bool,
) -> Result<()> {
    // Refused before anything is read: nothing is written over unasked.
    if let Some(out) = out.filter(|o| !force && o.exists()) {
        anyhow::bail!("{} exists; pass --force to replace it", out.display());
    }
    let reader = crate::local::Reader::open().await?;
    let id = ids::change(&reader, change).await?;
    let v: serde_json::Value = reader
        .get(&format!("/api/changes/{id}/certificate"))
        .await
        .with_context(|| format!("no certificate for `{id}`"))?;
    if let Some(e) = super::said_error(&v) {
        anyhow::bail!("{e}");
    }
    // Unfinished work is not an error for the Markdown: it says where it is.
    if !json && !sign {
        let md = v["markdown"].as_str().unwrap_or("");
        match out {
            None => println!("{md}"),
            Some(out) => {
                std::fs::write(out, format!("{md}\n"))
                    .with_context(|| format!("writing {}", out.display()))?;
                println!("wrote {}", out.display());
            }
        }
        return Ok(());
    }
    // The statement, or the sentence saying why there is none — an exit,
    // never an object that is not a statement.
    let st = &v["statement"];
    if let Some(why) = st[crate::core::certificate::NO_STATEMENT].as_str() {
        anyhow::bail!("{why}");
    }
    if st.get("_type").is_none() {
        anyhow::bail!("the host gave no statement for `{id}`");
    }
    let bytes = crate::core::attest::serialize(st);
    let Some(out) = out else {
        print!("{}", String::from_utf8_lossy(&bytes));
        return Ok(());
    };
    if !sign {
        std::fs::write(out, &bytes).with_context(|| format!("writing {}", out.display()))?;
        match json {
            true => crate::cli::print_json(&serde_json::json!({ "statement": out }))?,
            false => println!("wrote {}", out.display()),
        }
        return Ok(());
    }
    let key = crate::sign::SigningKey::from_global()?;
    let recipe = crate::sign::sign_to(out, &bytes, &key, force)?;
    let sig = format!("{}.sig", out.display());
    if json {
        return crate::cli::print_json(&recipe.json(&out.display().to_string(), &sig));
    }
    let dir = out
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let which = match crate::sign::fingerprint(&recipe.public_key, dir) {
        Some(fp) => format!("{} {fp}", recipe.public_key.split(' ').next().unwrap_or("")),
        None => recipe
            .public_key
            .split(' ')
            .next()
            .unwrap_or("")
            .to_string(),
    };
    println!(
        "wrote {} and {sig} — signed with {which} (your global user.signingkey)\n",
        out.display()
    );
    println!("To check it without Devplane, in the directory holding both files:\n");
    print!("{}", recipe.markdown());
    Ok(())
}

/// One change in full: where it is, what it cost, and what its checks said.
fn print_change(w: &ChangeView<'_>) {
    let c = &w.change;
    println!("{}  {}", paint(BOLD, &c.title), paint(DIM, c.id.as_str()));
    // What the change's own diff weakened follows the word on the same line,
    // never folded into it.
    let qualified = match w.qualifier.says.is_empty() {
        true => String::new(),
        false => format!(
            " · {}",
            paint(
                render::YELLOW,
                &format!("{} ({} unseen)", w.qualifier.says, w.qualifier.unseen)
            )
        ),
    };
    let beside = |s: Option<&str>| match s {
        Some(s) => format!(" — {}", paint(render::YELLOW, s)),
        None => String::new(),
    };
    println!(
        "  state      {} {}{qualified}{}{}",
        w.state.glyph(),
        w.state.as_str(),
        beside(w.waiting_says.as_deref()),
        beside(w.in_place_says.as_deref())
    );
    if let Some(at) = c.archived_at {
        println!("  archived   {at}");
    }
    // The agent's claim: after the weakened checks on the state line, before
    // the gates, and never in the gates' words.
    if let Some(claim) = &w.stop_claim {
        println!("  claim      {}", paint(render::YELLOW, &claim.says));
        for line in claim.quoted.lines() {
            println!("             {line}");
        }
        println!(
            "             {}",
            paint(
                DIM,
                &format!(
                    "mark it read: devplane change claim-read {id}   ·   answer the agent: \
                     devplane change resume {id}, then devplane change prompt {id} \"<your words>\"",
                    id = c.id
                )
            )
        );
    }
    println!("  shape      {}", w.shape_says);
    println!(
        "  gates      {}",
        paint(standing_colour(&w.standing), &w.standing_says)
    );
    // Beside the standing and never in it: how many tests ran, whether the
    // evidence tells the change from its base, and each run. Uncoloured.
    if let Some(b) = &w.base_checks {
        for (label, text) in b.show_lines() {
            println!("  {label:<10} {text}");
        }
    }
    if let Some(b) = &c.branch {
        println!("  branch     {b}");
    }
    if let Some(d) = &c.worktree {
        println!("  worktree   {}", d.display());
    }
    // The spec and its fingerprint when the gate last ran; the path alone
    // stops meaning anything once the file moves.
    if let Some(sp) = &c.spec {
        let stamp = w.gate.as_ref().and_then(|g| g.spec.as_ref());
        match stamp {
            Some(s) => match &s.fingerprint {
                Some(fp) => {
                    let over = match s.files > 1 {
                        true => format!(" over {} documents", s.files),
                        false => String::new(),
                    };
                    println!(
                        "  spec       {sp} {}",
                        paint(DIM, &format!("at {fp}{over}"))
                    )
                }
                None => println!(
                    "  spec       {sp} {}",
                    paint(render::RED, "— not there when the gate ran")
                ),
            },
            None => println!(
                "  spec       {sp} {}",
                paint(DIM, "— no gate has run against it yet")
            ),
        }
        // The spec's task list as of the gate run, counted, never judged.
        if let Some(s) = stamp.filter(|s| s.tasks_total > 0) {
            let left = s.tasks_total.saturating_sub(s.tasks_done);
            println!(
                "             {} {}",
                paint(BOLD, &format!("{}/{} tasks", s.tasks_done, s.tasks_total)),
                match left {
                    0 => paint(DIM, "every box ticked"),
                    _ => paint(
                        render::YELLOW,
                        &format!("{left} still open in the specification")
                    ),
                }
            );
        }
        if let Some(q) = stamp.map(|s| s.open_questions).filter(|q| *q > 0) {
            println!(
                "             {}",
                paint(
                    render::YELLOW,
                    &format!(
                        "{q} unanswered question{} in the specification",
                        if q == 1 { "" } else { "s" }
                    )
                )
            );
        }
    }
    // Ticked (what the agent wrote) and verified (a gate exit over a run that
    // was sent the task) are never merged. Ticked-but-unsent is listed.
    if let Some(says) = &w.counts_says {
        println!("  tasks      {}", paint(BOLD, says));
        let unsent = w
            .counts
            .as_ref()
            .map(|c| c.ticked_unsent.as_slice())
            .unwrap_or_default();
        if !unsent.is_empty() {
            println!(
                "             {}",
                paint(
                    render::YELLOW,
                    &format!("ticked, sent to nobody: {}", unsent.join(", "))
                )
            );
        }
        for row in &w.token_rows {
            println!("  {:<10} {}", row.row.token, row.says);
        }
    }
    for d in &w.drifts {
        println!("  drift      {}", paint(render::YELLOW, &d.says));
        println!(
            "             {}",
            paint(
                DIM,
                &format!(
                    "devplane change drift {} --run {} --accept | --tell",
                    c.id.as_str(),
                    d.drift.run.as_str()
                )
            )
        );
    }
    if !c.runs.is_empty() {
        println!(
            "  runs       {}",
            c.runs
                .iter()
                .map(|r| clip(r.as_str(), 12))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if c.feedback_rounds > 0 {
        println!("  handed back {} time(s)", c.feedback_rounds);
    }
    // A zero cost usually means the agent does not report one, so a
    // `[budget]` ceiling cannot bite; `$0.00` would hide that.
    match c.cost_usd {
        cost if cost > 0.0 => println!("  cost       ${cost:.2}"),
        _ => println!(
            "  cost       {}",
            paint(
                DIM,
                "not reported by this agent — a [budget] max_usd cannot bite"
            )
        ),
    }
    // Why it stopped, from the row: several reasons reach `failed`.
    if let Some(stopped) = &c.stopped {
        let line = match stopped {
            Stopped::OverBudget { spent_usd, bound } => {
                format!("stopped at ${spent_usd:.2}: {bound}")
            }
            Stopped::GateFailed { gate } => {
                format!("{gate} failed and the feedback budget is spent")
            }
            Stopped::Broken { detail } => format!("the change could not continue: {detail}"),
        };
        println!("  {}", paint(render::YELLOW, &line));
    }
    if let Some(pr) = &c.pull_request {
        println!(
            "  pull req   #{} {} {}",
            pr.number,
            pr.status,
            paint(DIM, &pr.url)
        );
    }
    if let Some(setup) = &c.setup {
        println!(
            "\n{} {}",
            paint(BOLD, "setup"),
            match setup.passed() {
                true => paint(render::GREEN, "ok"),
                false => paint(render::RED, "failed"),
            }
        );
    }

    let last_passed = w.gate.as_ref().is_some_and(|g| g.passed);
    let gates = &c.gates;
    for (n, g) in gates.iter().enumerate().rev().take(3) {
        let passed = last_passed && n + 1 == gates.len();
        println!(
            "\n{} {} {}",
            paint(BOLD, &g.gate),
            paint(DIM, &format!("attempt {} of {}", g.attempt, gates.len())),
            match passed {
                true => paint(render::GREEN, "passed"),
                false => String::new(),
            }
        );
        for cmd in &g.commands {
            // Exited non-zero, timed out, never started and undeterminable
            // are different; only the first is a verdict about the change.
            let (glyph, colour, note) = match &cmd.outcome {
                Outcome::Exited { code: 0 } => ("✓", render::GREEN, String::new()),
                Outcome::Exited { code } => ("✗", render::RED, format!("exit {code}")),
                Outcome::TimedOut { after_secs } => {
                    ("⏱", render::RED, format!("timed out after {after_secs}s"))
                }
                Outcome::NeverStarted { reason } => ("–", DIM, format!("never started: {reason}")),
                Outcome::Unknown { reason } => {
                    ("?", DIM, format!("could not be determined: {reason}"))
                }
            };
            println!(
                "  {} {} {}",
                paint(colour, glyph),
                cmd.command,
                paint(DIM, &note)
            );
            for f in cmd.failures.iter().take(8) {
                println!("      {}", paint(DIM, f));
            }
        }
    }

    // The agent's own account, printed only beside the measured lines and
    // judged by neither: the reader decides.
    if let Some(claim) = &w.claim {
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

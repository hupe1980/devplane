//! The queue of things being asked of a person, and answering them.

use super::raw;
use crate::render::{BOLD, DIM, clip, level_marker, paint};
use crate::{client, render};
use anyhow::Result;

pub async fn cmd_inbox(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    // **Running the command is reading it.** The output goes to somebody's
    // screen, which is the whole of what the boundary measures.
    let body = raw(&c, "/api/inbox?read=true").await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let items: Vec<render::InboxItem> =
        serde_json::from_value(body.get("items").cloned().unwrap_or_default()).unwrap_or_default();
    let close = body
        .get("close")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    // **The boundary, above everything.** One hairline saying how long it has
    // been, and nothing at all where there is no previous look — there is no
    // *last* to be since, and a line reading `0m` would be inventing one.
    if let Some(since) = close.get("since_last_look").and_then(|v| v.as_str()) {
        println!(
            "{}",
            paint(DIM, &format!("since you last looked · {since}"))
        );
        println!();
    }

    let folded: Vec<render::InboxSummary> =
        serde_json::from_value(body.get("folded").cloned().unwrap_or_default()).unwrap_or_default();
    let inhibited: Vec<render::InboxInhibited> =
        serde_json::from_value(body.get("inhibited").cloned().unwrap_or_default())
            .unwrap_or_default();

    // **Empty means nothing was raised**, not merely that nothing is listed.
    // Printing the close under a screen of summary rows would be telling
    // somebody the day was quiet because the surface tidied it.
    if items.is_empty() && folded.is_empty() && inhibited.is_empty() {
        render_close(&close);
        return Ok(());
    }
    for i in &items {
        // **The word carries it, the colour only helps** (the design system says so,
        // and the rule is stated there). Red and amber cannot be told apart
        // under deuteranopia at any usable lightness, so nothing in this product
        // may be distinguishable by colour alone — and *new since you last
        // looked* is exactly the distinction somebody scanning at 09:00 needs.
        //
        // Absent, not blank, where there is no boundary: a machine with no
        // previous look marks nothing, because there is nothing to be new to.
        let fresh = match i.new_to_you {
            true => format!(" {}", paint(render::GREEN, "new")),
            false => String::new(),
        };
        println!(
            "{} {} {}{}",
            level_marker(&i.level),
            paint(BOLD, &i.title),
            paint(DIM, &format!("[{}]", i.kind)),
            fresh
        );
        if let Some(d) = &i.detail {
            // **Per line, and indented per line.** A detail is prose an agent
            // or this product wrote and several kinds write more than one line
            // of it; clipping the whole thing as if it were one string cut the
            // `gate_down` row mid-path on its second line and left the third
            // hard against the margin, so the most alarming item in the inbox
            // was also the least readable one.
            for line in d.lines() {
                // A blank line in the middle keeps its blank; five spaces
                // followed by nothing is trailing whitespace somebody's diff
                // will complain about and nobody can see.
                match line.trim().is_empty() {
                    true => println!(),
                    false => println!("     {}", clip(line, 100)),
                }
            }
        }
        for (n, o) in i.options.iter().enumerate() {
            println!("     {}. {}", n + 1, o.label);
        }
        // The rule to paste, and where. A pattern only where this machine has
        // seen enough of the family to have a count behind it — being
        // interrupted once says this command needed a decision and says
        // nothing about the shape of the ones like it.
        //
        // **Printed, never written.** The rules are committed files reviewed
        // like code, and an agent here runs as the same user, so the handover
        // is a paste and that is the whole of it.
        if let Some(o) = &i.offer {
            let scope = match o.basis {
                crate::core::offer::Basis::Family => {
                    format!(
                        "covers {}{} calls like it",
                        o.covers,
                        if o.more { "+" } else { "" }
                    )
                }
                crate::core::offer::Basis::Call => "this call only".to_string(),
            };
            println!(
                "     {} {}",
                paint(DIM, "never asked again:"),
                paint(render::GREEN, &o.pasteable())
            );
            println!(
                "     {}",
                paint(
                    DIM,
                    &format!("{scope} · paste into {} {}", o.file, o.section)
                )
            );
        } else if let Some(no) = &i.no_offer {
            println!("     {}", paint(DIM, &no.sentence));
        }
        // A work item has no run of its own once the agent is gone, and
        // printing `run ` followed by nothing helps nobody.
        let subject = match (i.work_id.as_deref(), i.run_id.as_deref()) {
            (Some(w), _) => format!("work {}", clip(w, 14)),
            (None, Some(r)) if !r.is_empty() => format!("run {}", clip(r, 12)),
            _ => String::new(),
        };
        // An item about the machine has neither a subject nor an action, and
        // `     · ` is a line that says nothing and looks like a bug.
        let trailer = match (subject.is_empty(), i.actions.is_empty()) {
            (true, true) => String::new(),
            (true, false) => i.actions.join(", "),
            (false, true) => subject,
            (false, false) => format!("{subject} · {}", i.actions.join(", ")),
        };
        if !trailer.is_empty() {
            println!("     {}", paint(DIM, &trailer));
        }
        if let Some(url) = &i.url {
            println!("     {}", paint(DIM, url));
        }
        // The one action whose command a person cannot guess, spelled out for
        // the same reason `decide` is: it is offered exactly when it will work.
        if i.actions.iter().any(|a| a == "resume")
            && let Some(w) = &i.work_id
        {
            println!("     {}", paint(DIM, &format!("devplane work resume {w}")));
        }
        // Spell out the command only when Devplane can actually run it. For a
        // session it merely watches there is nothing to answer from here, and
        // printing a command that would fail is worse than printing none.
        //
        // **One command for both kinds now**, because the ask knows which it
        // is. There used to be two, chosen here by guessing from `kind` — and
        // the question branch printed the option's *label* where the protocol
        // wanted its *value*, which worked against the captured fixture and
        // against no real agent at all.
        if let Some(ask) = &i.ask {
            println!(
                "     {}",
                paint(DIM, &answer_command(ask, &i.kind, &i.options))
            );
        }
    }

    // **What was folded, and what it is of.** Nothing is hidden: each row says
    // the kind, the project and the count, and names the command that opens it.
    if !folded.is_empty() {
        println!();
        for f in &folded {
            println!(
                "{} {} {}",
                level_marker(&f.level),
                paint(BOLD, &format!("{} × {}", f.count, f.kind)),
                paint(
                    DIM,
                    &match &f.project {
                        Some(p) => format!("in {p}"),
                        None => "across projects".to_string(),
                    }
                )
            );
        }
        println!(
            "{}",
            paint(
                DIM,
                "     folded because the list is long — `devplane inbox --json` has every id"
            )
        );
    }

    // Symptoms counted on the row that explains them. Printed under the list
    // rather than beside each cause, because the cause is already in it.
    if !inhibited.is_empty() {
        println!();
        for s in &inhibited {
            println!(
                "{}",
                paint(
                    DIM,
                    &format!(
                        "     {} more {} — {}",
                        s.count,
                        plural_items(s.count),
                        s.because
                    )
                )
            );
        }
    }

    Ok(())
}

pub async fn cmd_attention(days: i64, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = raw(&c, &format!("/api/attention?days={days}")).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    // **The seat's own number, printed before the per-kind table.**
    //
    // `kinds` answers *is the inbox worth reading*; this answers *is anything
    // reaching you at all*. Different denominators, and only the second can
    // say the product is not working.
    //
    // **The measurement and the citation are visually separate and that is the
    // whole design.** The paper this idea came from defines vacuous oversight
    // over residual risk, needing an error rate Devplane cannot observe; the
    // ratio is one input to that model. So the first line is a fact this
    // product owns end to end, and anything about a threshold is marked as
    // somebody else's result with its identifier attached.
    if let Some(o) = v.get("oversight").filter(|o| !o.is_null()) {
        let n = |k: &str| o.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
        if let Some(sentence) = o.get("sentence").and_then(|s| s.as_str()) {
            let (answered, total) = (n("answered"), n("total"));
            // Red only when nothing at all reached a person. A low ratio is
            // not a failing grade — this product does not grade — but nought
            // of a large number is the one case worth the contrast.
            let colour = if answered == 0 && total > 0 {
                render::RED
            } else {
                BOLD
            };
            println!("{}", paint(colour, sentence));
            if n("asked") == 0 && n("unattended") > 0 {
                // Only when the attention log agrees. This sentence once
                // printed above a table showing sixty-nine raised permissions,
                // because the two numbers came from different tables.
                println!(
                    "{}",
                    paint(
                        DIM,
                        "Not one of them was put in front of you: every session here \
                         is in a mode that decides on its own. `devplane modes` says which."
                    )
                );
            }
            println!(
                "{}",
                paint(
                    DIM,
                    "There is no threshold for this ratio on its own. The published \
                     criterion (arXiv:2607.28317) is over residual risk and needs an \
                     error rate nothing here can observe; this is the count, not a verdict."
                )
            );
            println!();
        }
    }

    let empty = serde_json::Map::new();
    let kinds = v.get("kinds").and_then(|k| k.as_object()).unwrap_or(&empty);
    if kinds.is_empty() {
        println!(
            "{}",
            paint(
                DIM,
                &format!("Nothing has reached the inbox in the last {days} days.")
            )
        );
        return Ok(());
    }
    let n = |o: &serde_json::Value, k: &str| o.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    println!(
        "{}",
        paint(
            BOLD,
            &format!(
                "{:<18}{:>7}{:>8}{:>11}{:>11}{:>7}{:>8}{:>8}{:>9}",
                "kind",
                "raised",
                "acted",
                "dismissed",
                "elsewhere",
                "open",
                "acted",
                "folded",
                "wrongly"
            )
        )
    );
    let mut rows: Vec<(&String, &serde_json::Value)> = kinds.iter().collect();
    rows.sort_by_key(|(_, v)| -n(v, "raised"));
    for (kind, st) in rows {
        let (raised, acted, dismissed, elsewhere, open) = (
            n(st, "raised"),
            n(st, "acted"),
            n(st, "dismissed"),
            n(st, "elsewhere"),
            n(st, "open"),
        );
        // A kind nobody has resolved yet and a kind everybody ignores are
        // opposite facts; they must not print the same — and the daemon is
        // where that rule lives. This recomputed the ratio *and* the rule from
        // the raw counts, which is one rule in two places and only one of them
        // tested.
        let share = match st.get("acted_share").and_then(|s| s.as_f64()) {
            Some(f) => format!("{:.0}%", 100.0 * f),
            None => paint(DIM, "—"),
        };
        // **How often this kind was summarised rather than listed**, and how
        // often that turned out to be wrong. A kind always folded and never
        // acted on is one nobody needed as a row; a kind folded and then acted
        // on once opened is one the fold was standing in front of.
        let folded = n(st, "folded");
        let wrongly = n(st, "folded_then_acted");
        let f = match folded {
            0 => paint(DIM, "—"),
            v => v.to_string(),
        };
        let wr = match wrongly {
            0 => paint(DIM, "—"),
            v => paint(render::YELLOW, &v.to_string()),
        };
        println!(
            "{kind:<18}{raised:>7}{acted:>8}{dismissed:>11}{elsewhere:>11}{open:>7}{share:>8}{}{}",
            crate::render::pad_left(&f, 8),
            crate::render::pad_left(&wr, 9)
        );
    }
    println!(
        "\n{}",
        paint(
            DIM,
            "acted = you used one of the item's own actions · dismissed = you snoozed it · \
             elsewhere = it stopped asking on its own\nfolded = summarised rather than listed · \
             wrongly = folded, then acted on once opened — the fold was in the way",
        )
    );
    // **Said only when it is true.** How often an agent asks is a property of
    // the model — implicit escalation thresholds differ markedly by family and
    // the models' own confidence is miscalibrated per family — so a `raised`
    // column spanning two vendors is two escalation policies added together.
    // The ratio beside it is about the person and is unaffected; the count is
    // not, and a reader comparing weeks deserves to know which is which.
    //
    // A caveat printed on every run is a caveat nobody reads, so a machine with
    // one vendor never sees this line.
    let agents: Vec<&str> = v["agents"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if agents.len() > 1 {
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "raised spans {} — how often an agent asks is a property of the model, so \
                     compare the ratios rather than the counts",
                    agents.join(" and ")
                )
            )
        );
    }
    Ok(())
}

pub async fn cmd_say(run: &str, prompt: String) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v: serde_json::Value = c
        .post_json(
            &format!("/api/runs/{run}/prompt"),
            &serde_json::json!({ "text": prompt }),
        )
        .await?;
    match v.get("error").and_then(|e| e.as_str()) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!("{}", paint(DIM, "sent")),
    }
    Ok(())
}

pub async fn cmd_snooze(id: &str, minutes: i64, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    // Work and runs snooze separately, and the inbox lists both — so the id a
    // person copied off it decides which, rather than making them know. Work
    // ids are the ones this product mints, and they say so.
    let what = if id.starts_with("w-") { "work" } else { "runs" };
    let v: serde_json::Value = c
        .post(&format!("/api/{what}/{id}/snooze?minutes={minutes}"))
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else if minutes == 0 {
        println!(
            "{} — its items are back in the inbox.",
            paint(BOLD, "Un-snoozed")
        );
    } else {
        println!("Quiet for {minutes} minutes.");
    }
    Ok(())
}

/// Answers something an agent asked, by the ask's own token.
///
/// **One command for both kinds.** There were two — `decide` for a permission
/// and `answer` for a question — and a person had to know which protocol
/// channel a thing had arrived on before they could reply to it. The row knows;
/// the caller says what the person chose, and the daemon validates it against
/// the row.
pub async fn cmd_answer(
    ask: &str,
    allow: bool,
    deny: bool,
    option: Option<String>,
    custom: Option<String>,
    field: Option<String>,
) -> Result<()> {
    if !allow && !deny && option.is_none() && custom.is_none() {
        anyhow::bail!(
            "say what the answer is: --allow, --deny, --option '<what the agent offered>' or \
             --custom '<your words>'"
        );
    }
    let c = client::Client::connect_or_start().await?;
    let decision = match (allow, deny) {
        (true, _) => Some("allow"),
        (_, true) => Some("deny"),
        _ => None,
    };
    let v: serde_json::Value = c
        .post_json(
            &format!("/api/asks/{ask}/answer"),
            &serde_json::json!({
                "decision": decision,
                "option": option,
                "custom": custom,
                "field": field,
                "from": "cli",
            }),
        )
        .await?;
    match v.get("error").and_then(|e| e.as_str()) {
        Some(e) => anyhow::bail!("{e}"),
        // **What became of it, not just "answered".** Whether the answer
        // reached a waiting agent, went into a resumed session, or is recorded
        // and undelivered is the difference between the feature working and the
        // feature having been polite about failing.
        None => println!(
            "{}  {}",
            paint(render::GREEN, "answered"),
            paint(
                DIM,
                v.get("outcome")
                    .and_then(|o| o.as_str())
                    .unwrap_or_default()
            )
        ),
    }
    Ok(())
}

/// Everything an agent has asked, and what became of each one.
pub async fn cmd_asks(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = raw(&c, "/api/asks").await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = vec![];
    let open = v["open"].as_array().unwrap_or(&empty);
    let settled = v["settled"].as_array().unwrap_or(&empty);
    let abandoned = v["abandoned"].as_array().unwrap_or(&empty);
    let blind: Vec<&str> = v["blind_to"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|a| a.as_str())
        .collect();

    // **The empty state is two sentences, not one.** *Nothing was abandoned*
    // and *this vendor has no channel for it* are different facts, and printing
    // the first while meaning the second is a silence reading as a claim — the
    // defect the board's own empty state was written for, one surface further along.
    let blindness = || {
        if blind.is_empty() {
            return;
        }
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "Devplane cannot see abandoned questions for {} — the derivation is \
                     Claude Code's hook events and no other vendor documents an equivalent.",
                    blind.join(", ")
                )
            )
        );
    };

    if open.is_empty() && settled.is_empty() && abandoned.is_empty() {
        println!("{}", paint(DIM, "No agent has asked you anything yet."));
        blindness();
        return Ok(());
    }
    if open.is_empty() {
        println!("{}", paint(DIM, "Nothing is waiting on you."));
    }
    for a in open {
        println!(
            "{}  {}",
            paint(render::BOLD, a["id"].as_str().unwrap_or("")),
            a["message"].as_str().unwrap_or("")
        );
        println!(
            "{:>4}{}",
            "",
            paint(
                DIM,
                &format!(
                    "{} · {}",
                    a["kind"].as_str().unwrap_or(""),
                    a["deadline_says"].as_str().unwrap_or("")
                )
            )
        );
        println!(
            "{:>4}{}",
            "",
            paint(
                DIM,
                &format!("devplane answer {}", a["id"].as_str().unwrap_or(""))
            )
        );
    }
    for a in settled.iter().take(10) {
        println!(
            "{}  {}",
            paint(DIM, &clip(a["message"].as_str().unwrap_or(""), 48)),
            paint(DIM, a["outcome"].as_str().unwrap_or(""))
        );
    }

    // **The questions nobody answered**, from sessions Devplane only watches.
    // Ordered newest first and never ranked by anything the agent authored:
    // ranking these by importance means a model reading them.
    if !abandoned.is_empty() {
        println!();
        println!(
            "{}",
            paint(render::YELLOW, "Questions the agent asked and moved past")
        );
        for a in abandoned.iter().take(10) {
            println!("  {}", paint(BOLD, a["question"].as_str().unwrap_or("")));
            let what = match a["moved_on_to"].as_str() {
                Some(next) => format!("nobody answered — it did `{next}` instead"),
                None => "nobody answered — the session ended".to_string(),
            };
            println!("{:>4}{}", "", paint(DIM, &what));
        }
        if abandoned.len() > 10 {
            println!(
                "{:>4}{}",
                "",
                paint(DIM, &format!("and {} more", abandoned.len() - 10))
            );
        }
    } else {
        println!();
        println!("{}", paint(DIM, "No question was abandoned."));
    }
    blindness();
    Ok(())
}

/// Which projects are deciding without you.
///
/// **The one surface the 2026-09-19 measurement left standing.** Forty-nine
/// consecutive tool calls across four sessions put nothing in front of a
/// person, so a ledger of *what was decided without you* is a transcript of
/// everything. *Which of my repositories is in `auto`, and since when we knew*
/// is one short list, and nothing on this machine could answer it.
/// `1 item` / `2 items`, because a count in a sentence has to agree with it.
fn plural_items(n: usize) -> &'static str {
    match n {
        1 => "item",
        _ => "items",
    }
}

pub async fn cmd_modes(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = raw(&c, "/api/modes").await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = Vec::new();
    let projects = v
        .get("projects")
        .and_then(|p| p.as_array())
        .unwrap_or(&empty);
    // **Before the sessions, because it is true of all of them.** A timer that
    // answers a question in somebody's name is the same kind of fact as a
    // session running unsupervised, and it is the one this machine cannot
    // discover any other way: the vendor's own settings UI hides the row while
    // managed settings set it.
    // **Deserialised into the type the API serialised, never read key by key.**
    // This block used to reach for `c.get("file")` — a field renamed to
    // `where_set` three passes earlier — and `unwrap_or("")` printed a blank
    // line where the path belongs, so a person was told a clock was set and
    // never told where to change it. A rename is a compile error now.
    if let Some(c) = v
        .get("question_clock")
        .filter(|c| !c.is_null())
        .and_then(|c| serde_json::from_value::<crate::core::clock::ClockLine>(c.clone()).ok())
    {
        println!(
            "{}",
            match (c.answers_for_you, c.chosen_by_the_person) {
                // **`never` is not an alarm and not a finding — it is somebody
                // having said no.** Printed because it was written down, dim
                // because nothing about it needs acting on.
                (false, _) => paint(DIM, &c.says),
                (true, true) => paint(DIM, &c.says),
                // Somebody else set a clock on this person's attention. That is
                // the row this whole product exists to be able to show.
                (true, false) => paint(render::YELLOW, &c.says),
            }
        );
        println!("{}", paint(DIM, &format!("  set in {}", c.where_set)));
        println!();
    }
    if projects.is_empty() {
        println!("{}", paint(DIM, "No session is running."));
        return Ok(());
    }
    for p in projects {
        let name = p
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("(no project)");
        println!("\n{}", paint(BOLD, name));
        for s in p
            .get("sessions")
            .and_then(|s| s.as_array())
            .unwrap_or(&empty)
        {
            // Three outcomes, three colours, and the third is not the second.
            // A mode nobody here recognises is not "supervised" and it is not
            // "unsupervised" either — it is a question, and it prints as one.
            let (label, colour) = match (
                s.get("label").and_then(|l| l.as_str()),
                s.get("asks_a_person").and_then(|a| a.as_bool()),
            ) {
                (Some(l), Some(false)) => (l.to_string(), render::RED),
                (Some(l), Some(true)) => (l.to_string(), render::GREEN),
                (Some(l), None) => (format!("{l} (unknown to this build)"), render::YELLOW),
                // **A fourth case, and it is not "unknown to this build".** An
                // ACP agent declares its own mode as a string of its choosing.
                // There is no cross-vendor vocabulary for it and nothing maps
                // it onto the four a vendor documents, so **no `asks_a_person`
                // can be derived from it** — and it is attributed to the agent
                // rather than printed as though a settings file said it.
                //
                // Not coloured as an alarm: a mode this build cannot classify
                // is a fact about the protocol, not a finding about the person.
                (None, _)
                    if s.get("agent_mode")
                        .and_then(|m| m.as_str())
                        .is_some_and(|m| !m.is_empty()) =>
                {
                    (
                        format!(
                            "{} (the agent's own mode)",
                            s.get("agent_mode").and_then(|m| m.as_str()).unwrap_or("")
                        ),
                        DIM,
                    )
                }
                // Not a gap: the hook that fires on every tool call carries no
                // mode, so a session can be busy and genuinely not have said.
                (None, _) => ("not reported yet".to_string(), DIM),
            };
            let who = s
                .get("name")
                .and_then(|n| n.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| clip(s.get("run").and_then(|r| r.as_str()).unwrap_or("?"), 8));
            // "seen", never "since": nothing announces a mode change, so this
            // is when Devplane first heard it at this value.
            let seen = s
                .get("seen")
                .and_then(|t| t.as_str())
                .or_else(|| s.get("agent_mode_seen").and_then(|t| t.as_str()))
                .map(|t| format!("  {}", paint(DIM, &format!("seen {}", clip(t, 19)))))
                .unwrap_or_default();
            println!(
                "  {}{}{}",
                crate::render::pad(&who, 22),
                paint(colour, &label),
                seen
            );
            // **The session's own clock, under the mode it belongs to.**
            // `CLAUDE_AFK_TIMEOUT_MS` overrides the settings files and turns
            // auto-continue on even where they say `never`, so this is the one
            // line that can contradict the machine-wide one above — and the
            // person who needs it is the one whose file says `never`.
            //
            // Silence where nothing is set, deliberately: a caveat printed on
            // every row is a caveat nobody reads.
            if let Some(c) = s
                .get("question_clock")
                .filter(|c| !c.is_null())
                .and_then(|c| {
                    serde_json::from_value::<crate::core::clock::ClockLine>(c.clone()).ok()
                })
            {
                println!(
                    "  {:<22}{}",
                    "",
                    paint(
                        // Red for the session that answers instantly, because
                        // that is the one state where *a question waits for
                        // you* is untrue right now. Yellow where a clock
                        // answers at all. Dim for `never`, which is a session
                        // whose questions wait — the reassuring case, and one
                        // that was painted as an alarm until 2026-09-21.
                        match (c.immediate, c.answers_for_you) {
                            (true, _) => render::RED,
                            (false, true) => render::YELLOW,
                            (false, false) => DIM,
                        },
                        &c.says
                    )
                );
                println!(
                    "  {:<22}{}",
                    "",
                    paint(DIM, &format!("set in {}", c.where_set))
                );
            }
        }
    }
    let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let (total, unsup, unknown, unreported) = (
        n("sessions"),
        n("unsupervised"),
        n("unknown"),
        n("unreported"),
    );
    let unread = n("clock_unread");
    println!();
    // **Three sentences, because there are three facts.** Counting a session
    // that has not spoken as one running an exotic mode made sixteen idle
    // editors look like a security incident — found by running this against a
    // real machine, which is the only way it could have been found.
    if unsup > 0 {
        println!(
            "{}",
            paint(
                render::RED,
                &format!("{unsup} of {total} live session(s) decide without you.")
            )
        );
    }
    if unknown > 0 {
        println!(
            "{}",
            paint(
                render::YELLOW,
                &format!(
                    "{unknown} report a mode this build does not know, so whether \
                     anybody is asked cannot be said."
                )
            )
        );
    }
    if unreported > 0 {
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "{unreported} have not reported a mode yet — the hook that fires on \
                     every tool call does not carry one, so this fills in at their \
                     next prompt."
                )
            )
        );
    }
    // **The reassuring sentence, and it may not be printed over a clock.**
    // *Every session that has reported asks you* is about the permission mode,
    // and a session whose questions a timer closes is one where nobody is asked
    // whatever its mode says. Printing both — which is what this did on the
    // first run of the feature that reads the timer — is the surface
    // contradicting itself on one screen, and a person who catches that is
    // right to stop believing the rest of it.
    let clocked = projects
        .iter()
        .filter_map(|p| p.get("sessions").and_then(|s| s.as_array()))
        .flatten()
        .filter(|s| s.get("question_clock").is_some_and(|c| !c.is_null()))
        .count();
    if unsup == 0 && unknown == 0 && unreported < total && clocked == 0 {
        println!(
            "{}",
            paint(render::GREEN, "Every session that has reported asks you.")
        );
    }
    if clocked > 0 {
        println!(
            "{}",
            paint(
                render::YELLOW,
                &format!(
                    "{clocked} of {total} live session(s) have a clock that can answer \
                     for you, whatever their mode says."
                )
            )
        );
    }
    // **The coverage of the clock reading, and it is not the same fact as
    // *nothing is set*.** The environment can only be read by the hook that
    // runs as a child of the session, so a session that started before
    // `devplane connect` has no reading at all — and a surface that stayed
    // silent about those would be saying *your questions wait for you* about
    // sessions it has never looked at. Printed only when there are some.
    if unread > 0 {
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "{unread} of {total} started before Devplane could read their \
                     environment, so a `{}` on them would not be seen here. \
                     This fills in when they restart.",
                    crate::core::clock::ENV_KEY
                )
            )
        );
    }
    if unsup > 0 || unknown > 0 {
        println!(
            "{}",
            paint(
                DIM,
                "Devplane reads the mode and never sets it: change it where the session runs."
            )
        );
    }
    Ok(())
}

/// The command that answers one inbox item, spelled so that it runs.
///
/// **Pure, and separate from the printing, because it has been wrong twice.**
/// It printed `devplane decide` for a question, which fails with *"no permission
/// request is waiting"*; and then it printed the option's **label** where the
/// protocol wants its **value**, which is equal in the captured fixture and in
/// nothing else — so it passed every test and failed against real agents.
///
/// A command this product prints is a promise that it works, and the two ways
/// of breaking that promise are now pinned by tests rather than by a comment
/// asking the next person to be careful.
fn answer_command(ask: &str, kind: &str, options: &[crate::core::Choice]) -> String {
    match (kind, options.first().and_then(|o| o.id.as_deref())) {
        // The option's own id — the schema's `const`, not its `title`.
        ("question", Some(first)) => format!("devplane answer {ask} --option '{first}'"),
        // A question with no options is answered in prose; there is nothing to
        // pick, and offering `--option` would be offering an empty list.
        ("question", None) => format!("devplane answer {ask} --custom '<your words>'"),
        // A permission means a grant or a refusal however the agent spells its
        // options, so the shorthand is honest here and only here.
        _ => format!("devplane answer {ask} --allow"),
    }
}

/// The empty state.
///
/// **Longer than the list it replaces, on purpose.** A clear inbox is the one
/// moment a person has attention to spare, and the measured cost of making them
/// go and reconstruct the day themselves is 101.4 s against 45.4 s for being
/// told at a boundary. Every sentence here is the daemon's; this renders and
/// composes nothing.
fn render_close(close: &serde_json::Value) {
    println!("{}", paint(render::GREEN, "Clear."));

    let sentences: Vec<&str> = close
        .get("sentences")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    if sentences.is_empty() {
        // A day with nothing in it gets a sentence, never a table of zeroes.
        println!(
            "{}",
            paint(DIM, "Nothing needed you, and nothing was decided for you.")
        );
    } else {
        println!();
        for s in sentences {
            for line in crate::core::text::wrap(s, 76) {
                println!("  {line}");
            }
        }
    }

    if let Some(next) = close.get("next").and_then(|v| v.as_str()) {
        println!();
        println!("  {}", paint(render::YELLOW, &format!("next · {next}")));
    }

    if let Some(keeps) = close.get("keeps_running").and_then(|v| v.as_str()) {
        println!();
        for line in crate::core::text::wrap(keeps, 76) {
            println!("{}", paint(DIM, &line));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Choice;

    fn choice(id: &str, label: &str) -> Choice {
        Choice {
            id: Some(id.to_string()),
            label: label.to_string(),
            kind: None,
        }
    }

    /// The bug this exists for: the **id**, never the label. An agent whose
    /// option reads *Keep it* and answers to `keep_route` is the ordinary case
    /// everywhere except the fixture.
    #[test]
    fn a_question_is_answered_with_the_option_the_agent_will_accept() {
        let cmd = answer_command(
            "a1",
            "question",
            &[choice("keep_route", "Keep it — retains /v1/login")],
        );
        assert_eq!(cmd, "devplane answer a1 --option 'keep_route'");
        assert!(
            !cmd.contains("Keep it"),
            "the label is what a person reads and not what the protocol takes: {cmd}"
        );
    }

    /// The older bug, in the same two lines: a question is not a permission and
    /// `--allow` on one is a command that fails.
    #[test]
    fn a_question_is_never_answered_with_a_permission_shorthand() {
        assert!(!answer_command("a1", "question", &[choice("x", "X")]).contains("--allow"));
        assert!(answer_command("a1", "permission", &[]).contains("--allow"));
    }

    /// An agent that offered no options asked something wider than a list, and
    /// a command offering a choice from an empty list is one that cannot run.
    #[test]
    fn a_question_with_nothing_to_pick_is_answered_in_prose() {
        let cmd = answer_command("a1", "question", &[]);
        assert!(cmd.contains("--custom"), "{cmd}");
        assert!(!cmd.contains("--option"), "{cmd}");
    }

    /// An option a provider's own dialog owns carries no id, and Devplane
    /// cannot answer it. It must not be offered as a pick.
    #[test]
    fn an_option_with_no_id_is_not_offered_as_a_choice() {
        let unanswerable = Choice {
            id: None,
            label: "Yes".into(),
            kind: None,
        };
        let cmd = answer_command("a1", "question", std::slice::from_ref(&unanswerable));
        assert!(cmd.contains("--custom"), "{cmd}");
    }

    /// Every command this function can produce names the ask and nothing else.
    /// Addressing by run or by protocol request is what made an answer
    /// undeliverable the moment the session was gone.
    #[test]
    fn every_command_is_addressed_by_the_ask() {
        for (kind, opts) in [
            ("question", vec![choice("a", "A")]),
            ("question", vec![]),
            ("permission", vec![]),
        ] {
            let cmd = answer_command("tok", kind, &opts);
            assert!(cmd.starts_with("devplane answer tok "), "{cmd}");
            assert!(!cmd.contains("--request"), "{cmd}");
        }
    }
}

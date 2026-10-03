//! The queue of things being asked of a person, and answering them.

use super::raw_as;
use crate::render::{BOLD, DIM, clip, level_marker, paint};
use crate::{client, render};
use anyhow::Result;

/// How wide a detail line is printed; the only clip applied to it.
const DETAIL_WIDTH: usize = 100;

pub async fn cmd_inbox(json: bool, project: Option<&str>, needs_you: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    // Narrowing is done by the host, so the board and this command agree.
    let inbox = c.inbox(project, needs_you).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&inbox)?);
        return Ok(());
    }
    let crate::view::InboxView {
        items,
        folded,
        inhibited,
        narrowed,
        snoozed,
        snoozed_rows,
        close,
        held,
        ..
    } = inbox;

    // The "since you last looked" line: absent when there was no previous
    // look, and on a narrowed list (the host omits `close` there).
    if let Some(since) = close.as_ref().and_then(|c| c.since_last_look.as_deref()) {
        println!(
            "{}",
            paint(DIM, &format!("since you last looked · {since}"))
        );
        println!();
    }

    // Out of the list while it waits, so said here rather than hidden.
    if let Some(h) = &held {
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "held: {} → {} — sent to the agent at {} unless taken back in the window",
                    h.title, h.says, h.until
                )
            )
        );
    }

    if snoozed > 0 {
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "{snoozed} snoozed — `devplane inbox snooze <id> --minutes 0` brings one back"
                )
            )
        );
        for row in &snoozed_rows {
            println!("{}", paint(DIM, &format!("  {} — {}", row.title, row.says)));
        }
    }

    let left_out = narrowed.as_ref().map_or(0, |n| n.count);
    let no_such = narrowed.as_ref().is_some_and(|n| n.no_such_project);

    // Three empty states: no such project, nothing waiting in it, nothing
    // waiting anywhere. Told alike, a typo would read as good news.
    if items.is_empty() && folded.is_empty() && inhibited.is_empty() {
        if no_such {
            let known: Vec<String> = narrowed
                .as_ref()
                .map(|n| n.projects.clone())
                .unwrap_or_default();
            println!("No project matches {}.", paint(BOLD, project.unwrap_or("")));
            if !known.is_empty() {
                println!(
                    "{}",
                    paint(DIM, &format!("Waiting in: {}", known.join(", ")))
                );
            }
            println!("{}", paint(DIM, "devplane inbox lists every project."));
            return Ok(());
        }
        if project.is_some() || needs_you {
            println!("Nothing needs you here.");
            if left_out > 0 {
                println!(
                    "{}",
                    paint(
                        DIM,
                        &format!("{left_out} elsewhere. `devplane inbox` shows everything.")
                    )
                );
            }
            return Ok(());
        }
        render_close(close.as_ref());
        return Ok(());
    }
    for row in &items {
        let i = &row.item;
        let kind = i.kind.as_str();
        let actions: Vec<&str> = i.actions.iter().map(|a| a.as_str()).collect();
        let run_id = i.run_id.as_ref().map(|r| r.as_str());
        let change_id = i.change_id.as_ref().map(|w| w.as_str());
        // The word carries "new", colour only helps: red and amber are not
        // distinguishable under deuteranopia.
        let fresh = match row.new_to_you {
            true => format!(" {}", paint(render::GREEN, "new")),
            false => String::new(),
        };
        // Project and age, dim and after the kind; absent where there is none
        // (`gate_down` belongs to no project).
        let where_from = match &row.project_name {
            Some(p) if !p.is_empty() => format!(" {}", paint(DIM, p)),
            _ => String::new(),
        };
        let waited = {
            let secs = (jiff::Timestamp::now() - i.since).get_seconds().max(0);
            format!(" {}", paint(DIM, &render::ago(secs)))
        };
        println!(
            "{} {} {}{}{}{}",
            level_marker(i.level.as_str()),
            paint(BOLD, &i.title),
            paint(DIM, &format!("[{kind}]")),
            where_from,
            waited,
            fresh
        );
        if let Some(d) = &i.detail {
            // Clipped and indented per line; a clipped line points to the
            // command that shows the whole of it.
            let mut clipped = false;
            for line in d.lines() {
                // A blank line stays blank, without trailing whitespace.
                match line.trim().is_empty() {
                    true => println!(),
                    false => {
                        let shown = clip(line, DETAIL_WIDTH);
                        clipped |= shown != line;
                        println!("     {shown}");
                    }
                }
            }
            if clipped {
                let whole = match (run_id, change_id) {
                    (Some(r), _) if !r.is_empty() => format!("devplane run show {r}"),
                    (_, Some(w)) if !w.is_empty() => format!("devplane change show {w}"),
                    _ => "devplane inbox --json".to_string(),
                };
                println!(
                    "     {}",
                    paint(DIM, &format!("…  {whole} has the whole of it"))
                );
            }
        }
        // Why there is no yes-or-no, where there is not — the board's sentence.
        if let Some(where_to) = &i.answer_in {
            println!("     {}", paint(DIM, where_to));
        }
        for (n, o) in i.options.iter().enumerate() {
            println!("     {}. {}", n + 1, o.label);
        }
        // The rule to paste, and where. Printed, never written: rules are
        // committed files reviewed like code.
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
        // A change has no run of its own once the agent is gone.
        let subject = match (change_id, run_id) {
            (Some(w), _) => format!("change {}", clip(w, 14)),
            (None, Some(r)) if !r.is_empty() => format!("run {}", clip(r, 12)),
            _ => String::new(),
        };
        // A machine-wide item has neither subject nor action: print nothing.
        let trailer = match (subject.is_empty(), actions.is_empty()) {
            (true, true) => String::new(),
            (true, false) => actions.join(", "),
            (false, true) => subject,
            (false, false) => format!("{subject} · {}", actions.join(", ")),
        };
        if !trailer.is_empty() {
            println!("     {}", paint(DIM, &trailer));
        }
        if let Some(url) = &i.url {
            println!("     {}", paint(DIM, url));
        }
        // The one action whose command a person cannot guess.
        if actions.contains(&"resume")
            && let Some(w) = change_id
        {
            println!(
                "     {}",
                paint(DIM, &format!("devplane change resume {w}"))
            );
        }
        if actions
            .iter()
            .any(|a| *a == "tell_run" || *a == "accept_drift")
            && let (Some(w), Some(r)) = (change_id, run_id)
        {
            println!(
                "     {}",
                paint(
                    DIM,
                    &format!("devplane change drift {w} --run {r} --accept | --tell")
                )
            );
        }
        // Printed only when Devplane can deliver the answer; a watched session
        // has nothing to answer from here.
        if let Some(ask) = &i.ask {
            println!(
                "     {}",
                paint(DIM, &answer_command(ask.as_str(), kind, &i.options))
            );
        }
    }

    // What was folded: kind, project and count, and the command that opens it.
    if !folded.is_empty() {
        println!();
        for f in &folded {
            println!(
                "{} {} {}",
                level_marker(f.level.as_str()),
                paint(BOLD, &format!("{} × {}", f.count, f.kind.as_str())),
                paint(
                    DIM,
                    &match &f.project {
                        Some(p) => format!("in {}", p.as_str()),
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

    // Symptoms counted on the row that explains them, under the list.
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

    // A narrowed list always says how many it is not showing.
    if left_out > 0 {
        let where_: Vec<String> = narrowed
            .as_ref()
            .map(|n| n.projects.clone())
            .unwrap_or_default();
        println!();
        println!(
            "{}",
            paint(
                DIM,
                &match (left_out, where_.is_empty()) {
                    (1, true) => "1 more elsewhere. `devplane inbox` shows everything.".to_string(),
                    (n, true) => format!("{n} more elsewhere. `devplane inbox` shows everything."),
                    (1, false) => format!("1 more in {}.", where_.join(", ")),
                    (n, false) => format!("{n} more in {}.", where_.join(", ")),
                }
            )
        );
    }

    Ok(())
}

/// What the inbox raised over a window and what became of it, as the host's
/// diagnostics carry it.
#[derive(serde::Deserialize)]
pub(super) struct Attention {
    #[serde(default = "a_week")]
    days: i64,
    #[serde(default)]
    agents: Vec<String>,
    #[serde(default)]
    kinds: std::collections::BTreeMap<String, KindRow>,
}

fn a_week() -> i64 {
    7
}

/// One kind's counts, and the share the host computed from them.
#[derive(serde::Deserialize)]
struct KindRow {
    #[serde(flatten)]
    counts: crate::core::attention::KindStats,
    #[serde(default)]
    acted_share: Option<f64>,
}

/// `doctor`'s inbox section: per kind, what the last week raised and what
/// became of it — whether the inbox is worth reading at all.
pub(super) fn print_attention(v: &Attention) {
    let days = v.days;
    println!("\n{}", paint(BOLD, &format!("inbox, last {days} days")));
    let kinds = &v.kinds;
    if kinds.is_empty() {
        println!("  {}", paint(DIM, "nothing has reached the inbox"));
        return;
    }
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
    let mut rows: Vec<(&String, &KindRow)> = kinds.iter().collect();
    rows.sort_by_key(|(_, v)| -v.counts.raised);
    for (kind, row) in rows {
        let st = &row.counts;
        let (raised, acted, dismissed, elsewhere, open) =
            (st.raised, st.acted, st.dismissed, st.elsewhere, st.open);
        // "Nobody resolved it yet" and "everybody ignores it" differ; the host
        // computes `acted_share`, so the rule lives in one place.
        let share = match row.acted_share {
            Some(f) => format!("{:.0}%", 100.0 * f),
            None => paint(DIM, "—"),
        };
        // How often this kind was folded, and how often it was then acted on:
        // the latter is a fold that stood in the way.
        let folded = st.folded;
        let wrongly = st.folded_then_acted;
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
    // Ask frequency is a property of the model, so a `raised` count spanning
    // vendors mixes escalation policies. Said only when more than one vendor
    // is present.
    let agents = &v.agents;
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
}

/// The query a snooze sends: until the subject's next activity, or for
/// `minutes` (`0` brings everything back).
pub fn snooze_query(minutes: i64, until_activity: bool) -> String {
    match until_activity {
        true => "until=activity".to_string(),
        false => format!("minutes={minutes}"),
    }
}

pub async fn cmd_snooze(id: &str, minutes: i64, until_activity: bool, json: bool) -> Result<()> {
    let c = client::Client::connect_running().await?;
    let reader = crate::local::Reader::Host(c.clone());
    // A change id (`c-…`) snoozes the change; anything else is a run.
    let (what, id, subject) = match super::ids::change_or_run(&reader, id).await? {
        super::ids::Subject::Change(id) => ("changes", id, "the change"),
        super::ids::Subject::Run(id) => ("runs", id, "the session"),
    };
    let v: Snoozed = c
        .post(&format!(
            "/api/{what}/{id}/snooze?{}",
            snooze_query(minutes, until_activity)
        ))
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    match (until_activity, minutes) {
        (true, _) => println!("Snoozed until {subject} next reports activity."),
        (false, 0) => println!(
            "{} — its items are back in the inbox.",
            paint(BOLD, "Un-snoozed")
        ),
        (false, _) => println!("Quiet for {minutes} minutes."),
    }
    Ok(())
}

/// What a snooze answers: a run's kinds and until when, or a change's.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum Snoozed {
    Change {
        ok: bool,
        until: Option<String>,
        kinds: Vec<String>,
    },
    Run {
        snoozed_until: Option<String>,
        kinds: Vec<String>,
    },
}

/// What answering an ask returns: the ask as it now stands, or why the
/// answer was refused.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum Answered {
    Refused { error: String },
    Ask(Box<crate::view::AskView>),
}

/// Answers something an agent asked, by the ask's own token. The host
/// validates the choice against the ask, whether permission or question.
pub async fn cmd_answer(
    ask: &str,
    allow: bool,
    deny: bool,
    option: Option<String>,
    custom: Option<String>,
    field: Option<String>,
    json: bool,
) -> Result<()> {
    if !allow && !deny && option.is_none() && custom.is_none() {
        anyhow::bail!(
            "say what the answer is: --allow, --deny, --option '<what the agent offered>' or \
             --custom '<your words>'"
        );
    }
    // An agent may not answer its own permission: `--allow`, or an option
    // without a field, from inside an agent session is refused. `--deny` and
    // question answers go through. See `crate::hook::refuse_self_answer`.
    let allowing = allow || (option.is_some() && field.is_none() && !deny);
    if allowing && let Some(var) = crate::hook::agent_session() {
        anyhow::bail!("{}", crate::hook::refuse_self_answer(var));
    }
    let c = crate::local::Reader::open().await?;
    let ask = &super::ids::ask(&c, ask).await?;
    let decision = match (allow, deny) {
        (true, _) => Some("allow"),
        (_, true) => Some("deny"),
        _ => None,
    };
    let v: Answered = c
        .answer_as(
            ask,
            &serde_json::json!({
                "decision": decision,
                "option": option,
                "custom": custom,
                "field": field,
            }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    match v {
        Answered::Refused { error } => anyhow::bail!("{error}"),
        // The outcome: delivered to a waiting agent, into a resumed session,
        // or recorded and undelivered.
        Answered::Ask(a) => println!(
            "{}  {}",
            paint(render::GREEN, "answered"),
            paint(DIM, &a.outcome)
        ),
    }
    Ok(())
}

/// Everything an agent has asked, and what became of each one.
pub async fn cmd_inbox_all(json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let v: crate::view::AsksView = raw_as(&c, "/api/asks").await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let crate::view::AsksView {
        open,
        settled,
        abandoned,
        blind_to: blind,
    } = v;

    // "Nothing was abandoned" and "this vendor has no channel for it" are
    // different facts; `blindness` prints the second.
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
    for a in &open {
        println!("{}  {}", paint(render::BOLD, &a.id), a.message);
        println!(
            "{:>4}{}",
            "",
            paint(DIM, &format!("{} · {}", a.kind, a.deadline_says))
        );
        println!(
            "{:>4}{}",
            "",
            paint(DIM, &format!("devplane inbox answer {}", a.id))
        );
    }
    for a in settled.iter().take(10) {
        println!(
            "{}  {}",
            paint(DIM, &clip(&a.message, 48)),
            paint(DIM, &a.outcome)
        );
    }

    // Questions nobody answered in watched sessions, newest first and never
    // ranked by anything the agent wrote.
    if !abandoned.is_empty() {
        println!();
        println!(
            "{}",
            paint(render::YELLOW, "Questions the agent asked and moved past")
        );
        for a in abandoned.iter().take(10) {
            println!("  {}", paint(BOLD, &a.question));
            let what = match a.moved_on_to.as_deref() {
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

/// `1 item` / `2 items`.
fn plural_items(n: usize) -> &'static str {
    match n {
        1 => "item",
        _ => "items",
    }
}

/// Which projects are deciding without you, and what clock answers for you.
pub async fn cmd_modes(json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    // Deserialised into the API's own type, so a missing field fails to compile.
    let m: crate::core::world::Modes = raw_as(&c, "/api/modes").await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&m)?);
        return Ok(());
    }
    let projects = &m.projects;
    // The machine-wide question clock first: it applies to every session, and
    // managed settings can set it where the vendor's UI hides it.
    if let Some(c) = m.question_clock.as_ref() {
        println!(
            "{}",
            match (c.answers_for_you, c.chosen_by_the_person) {
                // `never` is somebody having said no: dim, nothing to act on.
                (false, _) => paint(DIM, &c.says),
                (true, true) => paint(DIM, &c.says),
                // Somebody else set a clock on this person's attention.
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
        let name = p.name.as_deref().unwrap_or("(no project)");
        println!("\n{}", paint(BOLD, name));
        for s in &p.sessions {
            // A mode this build does not recognise is neither supervised nor
            // unsupervised; it prints as a question.
            let (label, colour) = match (s.label.as_deref(), s.asks_a_person) {
                (Some(l), Some(false)) => (l.to_string(), render::RED),
                (Some(l), Some(true)) => (l.to_string(), render::GREEN),
                (Some(l), None) => (format!("{l} (unknown to this build)"), render::YELLOW),
                // A driven session's statement: the agent's own words for its
                // mode, and whether it asks, from that adapter's recording.
                (None, asks) if s.mode_statement.is_some() => (
                    s.mode_statement.clone().unwrap_or_default(),
                    match asks {
                        Some(false) => render::RED,
                        Some(true) => render::GREEN,
                        None => DIM,
                    },
                ),
                // An ACP agent's self-declared mode: no vendor vocabulary maps
                // it, so no `asks_a_person` is derived, and it is attributed to
                // the agent. Not an alarm.
                (None, _) if s.agent_mode.as_deref().is_some_and(|m| !m.is_empty()) => (
                    format!(
                        "{} (the agent's own mode)",
                        s.agent_mode.as_deref().unwrap_or("")
                    ),
                    DIM,
                ),
                // The per-tool-call hook carries no mode, so a busy session may
                // genuinely not have said.
                (None, _) => ("not reported yet".to_string(), DIM),
            };
            let who = s.name.clone().unwrap_or_else(|| clip(&s.run, 8));
            // "seen", not "since": nothing announces a mode change.
            let seen = s
                .seen
                .as_deref()
                .or(s.agent_mode_seen.as_deref())
                .map(|t| format!("  {}", paint(DIM, &format!("seen {}", clip(t, 19)))))
                .unwrap_or_default();
            println!(
                "  {}{}{}",
                crate::render::pad(&who, 22),
                paint(colour, &label),
                seen
            );
            // The session's own clock: `CLAUDE_AFK_TIMEOUT_MS` overrides the
            // settings files, so it can contradict the machine-wide line.
            // Silent where nothing is set.
            if let Some(c) = s.question_clock.as_ref() {
                println!(
                    "  {:<22}{}",
                    "",
                    paint(
                        // Red: answers instantly. Yellow: a clock answers.
                        // Dim: `never`, so questions wait.
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
    let (total, unsup, unknown, unreported) = (m.sessions, m.unsupervised, m.unknown, m.unreported);
    let unread = m.clock_unread;
    println!();
    // Unsupervised, unknown and unreported are three facts and three
    // sentences; a silent session is not an exotic mode.
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
    // The reassuring sentence is about permission mode, so it is withheld
    // whenever a clock can answer for the person.
    let clocked = projects
        .iter()
        .flat_map(|p| p.sessions.iter())
        .filter(|s| s.question_clock.is_some())
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
    // Sessions started before `devplane agent connect` have no clock reading, which
    // is not the same as "nothing set". Printed only when there are some.
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
/// Separate from printing so the tests below pin it.
fn answer_command(ask: &str, kind: &str, options: &[crate::core::Choice]) -> String {
    match (kind, options.first().and_then(|o| o.id.as_deref())) {
        // The option's own id — the schema's `const`, not its `title`.
        ("question", Some(first)) => format!("devplane inbox answer {ask} --option '{first}'"),
        // A question with no options is answered in prose.
        ("question", None) => format!("devplane inbox answer {ask} --custom '<your words>'"),
        // A permission is a grant or refusal however the agent spells options.
        _ => format!("devplane inbox answer {ask} --allow"),
    }
}

/// The empty state. Every sentence is the host's; this only lays them out.
fn render_close(close: Option<&crate::view::Close>) {
    println!("{}", paint(render::GREEN, "Clear."));
    let nothing = crate::view::Close::default();
    let close = close.unwrap_or(&nothing);
    if close.sentences.is_empty() {
        // A day with nothing in it gets a sentence, never a table of zeroes.
        println!(
            "{}",
            paint(DIM, "Nothing needed you, and nothing was decided for you.")
        );
    } else {
        println!();
        for s in &close.sentences {
            for line in crate::core::text::wrap(s, 76) {
                println!("  {line}");
            }
        }
    }

    if let Some(next) = &close.next {
        println!();
        println!("  {}", paint(render::YELLOW, &format!("next · {next}")));
    }

    if let Some(keeps) = &close.keeps_running {
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

    /// `--until-activity` asks the host for the subject's next activity,
    /// and is refused beside `--minutes`.
    #[test]
    fn a_snooze_until_activity_sends_no_minutes_and_refuses_them() {
        assert_eq!(snooze_query(60, true), "until=activity");
        assert_eq!(snooze_query(15, false), "minutes=15");
        assert_eq!(snooze_query(0, false), "minutes=0");
        use clap::Parser;
        let ok = crate::cli::Cli::try_parse_from([
            "devplane",
            "inbox",
            "snooze",
            "s1",
            "--until-activity",
        ]);
        assert!(ok.is_ok(), "{:?}", ok.err());
        let both = crate::cli::Cli::try_parse_from([
            "devplane",
            "inbox",
            "snooze",
            "s1",
            "--until-activity",
            "--minutes",
            "5",
        ]);
        assert!(
            both.is_err(),
            "minutes and activity are two different snoozes"
        );
    }

    /// The option's id, never its label: they differ for real agents.
    #[test]
    fn a_question_is_answered_with_the_option_the_agent_will_accept() {
        let cmd = answer_command(
            "a1",
            "question",
            &[choice("keep_route", "Keep it — retains /v1/login")],
        );
        assert_eq!(cmd, "devplane inbox answer a1 --option 'keep_route'");
        assert!(
            !cmd.contains("Keep it"),
            "the label is what a person reads and not what the protocol takes: {cmd}"
        );
    }

    /// A question is not a permission; `--allow` on one fails.
    #[test]
    fn a_question_is_never_answered_with_a_permission_shorthand() {
        assert!(!answer_command("a1", "question", &[choice("x", "X")]).contains("--allow"));
        assert!(answer_command("a1", "permission", &[]).contains("--allow"));
    }

    /// No options means an open question: nothing to pick from.
    #[test]
    fn a_question_with_nothing_to_pick_is_answered_in_prose() {
        let cmd = answer_command("a1", "question", &[]);
        assert!(cmd.contains("--custom"), "{cmd}");
        assert!(!cmd.contains("--option"), "{cmd}");
    }

    /// An option owned by a provider's own dialog has no id and is not offered.
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

    /// Every command is addressed by the ask, which outlives the session.
    #[test]
    fn every_command_is_addressed_by_the_ask() {
        for (kind, opts) in [
            ("question", vec![choice("a", "A")]),
            ("question", vec![]),
            ("permission", vec![]),
        ] {
            let cmd = answer_command("tok", kind, &opts);
            assert!(cmd.starts_with("devplane inbox answer tok "), "{cmd}");
            assert!(!cmd.contains("--request"), "{cmd}");
        }
    }
}

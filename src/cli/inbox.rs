//! The queue of things being asked of a person, and answering them.

use super::raw;
use crate::render::{BOLD, DIM, clip, level_marker, paint};
use crate::{client, render};
use anyhow::Result;

pub async fn cmd_inbox(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let items: Vec<render::InboxItem> = c.get("/api/inbox").await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&raw(&c, "/api/inbox").await?)?
        );
        return Ok(());
    }
    if items.is_empty() {
        println!("{}", paint(render::GREEN, "Nothing needs you."));
        return Ok(());
    }
    for i in &items {
        println!(
            "{} {} {}",
            level_marker(&i.level),
            paint(BOLD, &i.title),
            paint(DIM, &format!("[{}]", i.kind))
        );
        if let Some(d) = &i.detail {
            println!("     {}", clip(d, 100));
        }
        for (n, o) in i.options.iter().enumerate() {
            println!("     {}. {}", n + 1, o.label);
        }
        // A work item has no run of its own once the agent is gone, and
        // printing `run ` followed by nothing helps nobody.
        let subject = match (i.work_id.as_deref(), i.run_id.as_str()) {
            (Some(w), _) => format!("work {}", clip(w, 14)),
            (None, r) if !r.is_empty() => format!("run {}", clip(r, 12)),
            _ => String::new(),
        };
        println!(
            "     {}",
            paint(DIM, &format!("{subject} · {}", i.actions.join(", ")))
        );
        if let Some(url) = &i.url {
            println!("     {}", paint(DIM, url));
        }
        // The one action whose command a person cannot guess, spelled out for
        // the same reason `decide` is: it is offered exactly when it will work.
        if i.actions.iter().any(|a| a == "resume")
            && let Some(w) = &i.work_id
        {
            println!("     {}", paint(DIM, &format!("vibeplane work resume {w}")));
        }
        // Spell out the command only when Vibeplane can actually run it. For a
        // session it merely watches there is nothing to decide from here, and
        // printing a command that would fail is worse than printing none.
        if let Some(req) = &i.request_id {
            println!(
                "     {}",
                paint(
                    DIM,
                    &format!(
                        "vibeplane decide {} --request {} --decision allow",
                        i.run_id, req
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
                "{:<18}{:>7}{:>8}{:>11}{:>11}{:>7}{:>8}",
                "kind", "raised", "acted", "dismissed", "elsewhere", "open", "acted"
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
        // opposite facts; they must not print the same.
        let closed = acted + dismissed + elsewhere;
        let share = match closed {
            0 => paint(DIM, "—"),
            _ => format!("{:.0}%", 100.0 * acted as f64 / closed as f64),
        };
        println!(
            "{kind:<18}{raised:>7}{acted:>8}{dismissed:>11}{elsewhere:>11}{open:>7}{share:>8}"
        );
    }
    println!(
        "\n{}",
        paint(
            DIM,
            "acted = you used one of the item's own actions · dismissed = you snoozed it · \
             elsewhere = it stopped asking on its own",
        )
    );
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

pub async fn cmd_decide(
    run: &str,
    request: &str,
    decision: &str,
    option: Option<String>,
) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v: serde_json::Value = c
        .post_json(
            &format!("/api/runs/{run}/decide"),
            &serde_json::json!({
                "request_id": request,
                "decision": decision,
                "option_id": option,
            }),
        )
        .await?;
    match v.get("error").and_then(|e| e.as_str()) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!("{}", paint(render::GREEN, "answered")),
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

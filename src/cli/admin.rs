//! What happened: search across sessions, and the audit trail of decisions.

use super::{raw, urlencode};
use crate::render;
use crate::render::{DIM, clip, paint};
use anyhow::Result;

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
    let about = match about {
        Some(needle) => Some(super::ids::change_or_run(&c, needle).await?),
        None => None,
    };
    let about = about.as_ref().map(super::ids::Subject::id);
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

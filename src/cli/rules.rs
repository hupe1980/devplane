//! `devplane rules` — which registered repository is missing a rule.
//!
//! It writes nothing, and says so; `core::rules` explains why there is no
//! apply-to-all.

use super::raw;
use crate::render;
use crate::render::{BOLD, DIM, paint};
use anyhow::Result;

pub async fn cmd_rules(rule: Option<String>, ask: bool, json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let mut path = "/api/rules".to_string();
    if let Some(r) = rule.as_deref() {
        path.push_str(&format!("?rule={}", query_escape(r)));
        if ask {
            path.push_str("&ask=true");
        }
    }
    let v = raw(&c, &path).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    match v.get("asked").and_then(|a| a.as_str()) {
        Some(asked) => one_rule(&v, asked),
        None => disagreements(&v),
    }
    Ok(())
}

/// Percent-escapes a rule for a query value.
///
/// Not a general encoder: it escapes separators and leaves rule syntax such as
/// `Bash(curl:*)` legible in a log.
fn query_escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'('
            | b')'
            | b'*'
            | b':'
            | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// One rule, every project.
fn one_rule(v: &serde_json::Value, asked: &str) {
    let empty = Vec::new();
    let rows: Vec<crate::core::rules::Row> = v
        .get("rows")
        .and_then(|r| r.as_array())
        .unwrap_or(&empty)
        .iter()
        .filter_map(|r| serde_json::from_value(r.clone()).ok())
        .collect();

    if rows.is_empty() {
        println!(
            "{}",
            paint(
                DIM,
                "No project is registered. `devplane connect <path>` adds one."
            )
        );
        return;
    }

    println!("{}\n", paint(BOLD, asked));

    let width = rows
        .iter()
        .map(|r| r.project.len())
        .max()
        .unwrap_or(0)
        .max(7);
    let mut missing: Vec<&crate::core::rules::Row> = Vec::new();
    for r in &rows {
        use crate::core::rules::Coverage::*;
        // The word carries the state; colour only repeats it for a pipe's sake.
        let (word, colour) = match &r.coverage {
            Has => ("has it", render::GREEN),
            Covered { .. } => ("covered", render::GREEN),
            Missing => ("missing", render::YELLOW),
            Unreadable { .. } => ("unreadable", render::RED),
        };
        println!(
            "  {}  {}  {}  {}",
            render::pad(&r.project, width),
            render::pad(&paint(DIM, r.source.as_str()), 8),
            render::pad(&paint(colour, word), 10),
            paint(
                DIM,
                &match &r.coverage {
                    Covered { by } => format!("by `{by}`"),
                    Unreadable { why } => format!("{why} — run `devplane check`"),
                    _ => r.file.clone(),
                }
            ),
        );
        if matches!(r.coverage, Missing) {
            missing.push(r);
        }
    }

    if missing.is_empty() {
        println!(
            "\n{}",
            paint(DIM, "Every project already covers this call.")
        );
    } else {
        // The rule once, with every destination listed under it.
        println!("\n{}", paint(BOLD, "Paste this:"));
        println!("  {asked}");
        println!("\n{}", paint(DIM, "into:"));
        for r in &missing {
            println!(
                "  {}  {}",
                paint(DIM, r.section.as_deref().unwrap_or("")),
                r.file
            );
        }
    }
    footer(v);
}

/// What the projects disagree about.
fn disagreements(v: &serde_json::Value) {
    let empty = Vec::new();
    let rows: Vec<crate::core::rules::Spread> = v
        .get("disagreements")
        .and_then(|r| r.as_array())
        .unwrap_or(&empty)
        .iter()
        .filter_map(|r| serde_json::from_value(r.clone()).ok())
        .collect();

    let projects = v.get("projects").and_then(|p| p.as_u64()).unwrap_or(0);
    if projects < 2 {
        println!(
            "{}",
            paint(
                DIM,
                "Fewer than two projects are registered, so there is nothing to compare."
            )
        );
        return;
    }
    if rows.is_empty() {
        println!(
            "{}",
            paint(
                DIM,
                &format!("All {projects} projects hold the same rules. Nothing to show.")
            )
        );
        return;
    }

    println!(
        "{}\n",
        paint(BOLD, "Rules some projects have and others do not")
    );
    for s in &rows {
        println!(
            "  {}  {}",
            paint(BOLD, &s.rule),
            paint(DIM, s.source.as_str())
        );
        println!(
            "    {} {}",
            paint(render::GREEN, &format!("{:>2} have", s.held_by.len())),
            paint(DIM, &s.held_by.join(", "))
        );
        println!(
            "    {} {}",
            paint(
                render::YELLOW,
                &format!("{:>2} do not", s.missing_from.len())
            ),
            paint(DIM, &s.missing_from.join(", "))
        );
    }
    println!(
        "\n{}",
        paint(
            DIM,
            "`devplane rules '<rule>'` for one of them, with the text to paste."
        )
    );
    footer(v);
}

/// The two sentences every rendering ends with: that nothing was written, and
/// why there is no apply-to-all.
fn footer(v: &serde_json::Value) {
    for key in ["wrote_nothing", "why_no_apply_to_all"] {
        if let Some(line) = v.get(key).and_then(|s| s.as_str()) {
            println!("\n{}", paint(DIM, &wrapped(line, 78)));
        }
    }
}

/// Wraps a sentence at a column, on word boundaries.
fn wrapped(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut line = 0usize;
    for word in text.split_whitespace() {
        // `+ 1` for the preceding space. An over-long word overruns rather than
        // breaking, so a URL or rule stays copyable.
        if line > 0 && line + 1 + word.len() > width {
            out.push('\n');
            line = 0;
        } else if line > 0 {
            out.push(' ');
            line += 1;
        }
        out.push_str(word);
        line += word.len();
    }
    out
}

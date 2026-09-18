//! Every colour this interface renders is readable against the surface it sits
//! on, in both themes — **computed here, never listed**.
//!
//! The property this replaces read *"contrast is checked at AA for text and for
//! the state colours against both grounds"* and was **asserted by eye**. It was
//! wrong by 3.2:1 in the dark theme and 2.9:1 in the light one, on `--faint`,
//! which carries the cost and the context percentage — the two numbers this
//! persona scans most were the least legible things on the row, in both themes,
//! for every pass since the page existed.
//!
//! **A maintained list of approved pairs would fail the same way**, the first
//! time somebody adds a token and forgets to add its row. So this reads the
//! tokens out of the page itself and computes the ratio: a token that exists is
//! a token that is checked.

use std::collections::BTreeMap;

const PAGE: &str = include_str!("../ui/index.html");

/// The floor for text, and the floor for everything else.
///
/// Not named as a number anywhere but here. A specification that repeated it
/// would be a second home for a figure, and the two would disagree the first
/// time one moved.
const TEXT_FLOOR: f64 = 4.5;
const NON_TEXT_FLOOR: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Surface,
    /// A separator between things. Decorative: nothing about operating the
    /// interface depends on perceiving it, so no floor applies.
    Decoration,
    /// The boundary of a control. Held to the non-text floor, because an input
    /// whose edge cannot be seen is a control that cannot be found.
    Edge,
    Text,
}

/// Relative luminance, per the contrast definition.
///
/// A dozen lines rather than a crate, for the reason every other piece of
/// arithmetic in this tree has none: nothing belongs between a claim and its
/// evidence.
fn luminance(hex: &str) -> f64 {
    let h = hex.trim_start_matches('#');
    let full = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => h.to_string(),
        _ => panic!("`{hex}` is not a colour this check can read"),
    };
    let channel = |i: usize| {
        let v = u8::from_str_radix(&full[i..i + 2], 16).expect("two hex digits") as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4)
}

fn ratio(a: &str, b: &str) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

/// The tokens of one theme, read out of the page.
///
/// From the page, never from a copy here: a token that exists is a token that
/// is checked, and a duplicate table would go stale exactly the way the
/// accessibility section did.
fn theme(block: &str) -> BTreeMap<String, (String, Role)> {
    let mut out = BTreeMap::new();
    for line in block.lines() {
        let Some(rest) = line.trim().strip_prefix("--") else {
            continue;
        };
        let Some((name, tail)) = rest.split_once(':') else {
            continue;
        };
        let Some((value, comment)) = tail.split_once("/*") else {
            continue;
        };
        let role = match comment.trim().trim_end_matches("*/").trim() {
            "surface" => Role::Surface,
            "line" => Role::Decoration,
            "edge" => Role::Edge,
            "text" => Role::Text,
            other => panic!("`--{name}` declares an unknown role `{other}`"),
        };
        out.insert(
            name.trim().to_string(),
            (value.trim().trim_end_matches(';').to_string(), role),
        );
    }
    out
}

/// Every block that declares tokens, with the selector that introduces it.
///
/// There are three and they are two themes: the light default, the dark one
/// under the system preference, and the dark one under an explicit choice. The
/// last two carry the same values and **have to**, because a media query and an
/// attribute cannot be one selector — so they are compared rather than trusted.
fn token_blocks() -> Vec<(String, BTreeMap<String, (String, Role)>)> {
    let mut out = Vec::new();
    for (at, _) in PAGE.match_indices(":root") {
        let rest = &PAGE[at..];
        let Some(open) = rest.find('{') else { continue };
        let selector = rest[..open].trim().to_string();
        let Some(close) = rest.find('}') else {
            continue;
        };
        let body = &rest[open..close];
        if !body.contains("--bg:") {
            continue;
        }
        out.push((selector, theme(body)));
    }
    out
}

fn themes() -> Vec<(&'static str, BTreeMap<String, (String, Role)>)> {
    let blocks = token_blocks();
    assert!(
        blocks.len() >= 3,
        "expected a light block and two dark ones; found {}",
        blocks.len()
    );
    let light = blocks
        .iter()
        .find(|(sel, _)| !sel.contains("dark") && !sel.contains("light"))
        .expect("a default block")
        .1
        .clone();
    let dark = blocks
        .iter()
        .find(|(sel, _)| sel.contains("data-theme=\"dark\""))
        .expect("an explicit dark block")
        .1
        .clone();
    vec![("light", light), ("dark", dark)]
}

#[test]
fn the_two_ways_of_asking_for_dark_agree() {
    // A media query and an attribute cannot be one selector, so the values are
    // written twice — and two copies of a figure is how a document starts
    // lying. This is the check that stops them drifting.
    let blocks = token_blocks();
    let by_preference = blocks
        .iter()
        .find(|(sel, _)| sel.contains("not([data-theme=\"light\"])"))
        .expect("dark under the system preference");
    let by_choice = blocks
        .iter()
        .find(|(sel, _)| sel.contains("[data-theme=\"dark\"]") && !sel.contains("not("))
        .expect("dark by explicit choice");
    assert_eq!(
        by_preference.1, by_choice.1,
        "the two dark blocks have drifted apart: a person who chose dark and a \
         person whose system chose it would see different colours"
    );
}

#[test]
fn every_text_token_is_readable_on_every_surface_in_both_themes() {
    let mut failures = Vec::new();
    for (name, tokens) in themes() {
        let surfaces: Vec<(&String, &String)> = tokens
            .iter()
            .filter(|(_, (_, r))| *r == Role::Surface)
            .map(|(k, (v, _))| (k, v))
            .collect();
        assert!(!surfaces.is_empty(), "{name} declares no surface");

        for (token, (value, role)) in &tokens {
            let floor = match role {
                Role::Text => TEXT_FLOOR,
                Role::Edge => NON_TEXT_FLOOR,
                Role::Surface | Role::Decoration => continue,
            };
            for (surface, bg) in &surfaces {
                let r = ratio(value, bg);
                if r < floor {
                    // The failure names the token, the surface and the ratio.
                    // "Contrast failed" is not actionable at the moment
                    // somebody is changing a colour.
                    failures.push(format!(
                        "  {name}: --{token} ({value}) on --{surface} ({bg}) is {r:.2}:1, below {floor}:1"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "colours below the readability floor:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_token_is_defined_in_both_themes_with_the_same_role() {
    // A missing value fails rather than falling back, because a fallback
    // silently renders one theme wearing the other's colour.
    let t = themes();
    let (a_name, a) = &t[0];
    let (b_name, b) = &t[1];
    for (token, (_, role)) in a {
        let (_, other_role) = b
            .get(token)
            .unwrap_or_else(|| panic!("`--{token}` is in {a_name} and missing from {b_name}"));
        assert_eq!(
            role, other_role,
            "`--{token}` has different roles in the two themes"
        );
    }
    for token in b.keys() {
        assert!(
            a.contains_key(token),
            "`--{token}` is in {b_name} and missing from {a_name}"
        );
    }
}

// ── The first renderer moved out of the page ────────────────────────────────

#[test]
fn the_reason_pane_renders_and_escapes_what_it_is_given() {
    use devplane::render::{DecisionRow, reason_pane};

    let rows = vec![DecisionRow {
        at: "14:02".into(),
        actor: "policy".into(),
        action: r#"Bash(<img src=x onerror=alert(1)>)"#.into(),
        outcome: "allow".into(),
        reason: Some(r#"auto_allow = ["Bash(pnpm test *)"]"#.into()),
    }];

    let out = reason_pane(
        Some(("a <b>title</b>", "permission · blocking", None)),
        &rows,
    );
    let html = out.as_str();

    // The markup this module wrote is present…
    assert!(
        html.contains(r#"<div class="e">"#),
        "the row did not render"
    );
    assert!(html.contains("14:02"), "the time did not render");
    // …and nothing that came from outside it survives as markup.
    assert!(
        !html.contains("<img"),
        "a tool input reached the page as markup"
    );
    assert!(
        !html.contains("<b>title</b>"),
        "a title reached the page as markup"
    );
    assert!(
        html.contains("&lt;img"),
        "the tool input was dropped rather than escaped"
    );

    // Absence is distinguishable from zero.
    let empty = reason_pane(None, &[]);
    assert!(
        empty
            .as_str()
            .contains("Nothing has been decided about this yet"),
        "an empty pane must say so rather than rendering nothing"
    );
    assert!(
        !empty.as_str().contains(r#"class="e""#),
        "an empty pane rendered a row"
    );
}

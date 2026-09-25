//! Every colour the interface renders is readable against its surface in both
//! themes. Ratios are computed from the stylesheet's own tokens, so a new token
//! is checked without a list to maintain.

use std::collections::BTreeMap;

/// The palette. Values are read from it, never compared against a golden copy.
const PAGE: &str = include_str!("../ui/src/tokens.css");

/// The floor for text, and the floor for everything else. Stated only here.
const TEXT_FLOOR: f64 = 4.5;
const NON_TEXT_FLOOR: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Surface,
    /// A separator; decorative, so no floor applies.
    Decoration,
    /// A control's boundary, held to the non-text floor so the control can be found.
    Edge,
    Text,
}

/// Relative luminance, per the contrast definition.
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
fn theme(block: &str) -> BTreeMap<String, (String, Role)> {
    let mut out = BTreeMap::new();
    for line in block.lines() {
        let Some(rest) = line.trim().strip_prefix("--") else {
            continue;
        };
        let Some((name, tail)) = rest.split_once(':') else {
            continue;
        };
        // A token with no role would go unchecked, so it is refused, not skipped.
        let Some((value, comment)) = tail.split_once("/*") else {
            panic!(
                "`--{name}` declares no role: write `/* surface | line | edge | text */` beside it"
            );
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

/// One theme's tokens: name to (value, the role it plays).
type Tokens = BTreeMap<String, (String, Role)>;

#[test]
fn a_token_without_a_role_is_refused_rather_than_skipped() {
    let refused = std::panic::catch_unwind(|| {
        theme("    --bg: #fbfbfa;      /* surface */\n    --oops: #123456;\n")
    });
    let msg = match refused {
        Ok(_) => panic!("a token with no role comment was silently skipped"),
        Err(e) => e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default(),
    };
    assert!(msg.contains("--oops"), "the refusal names the token: {msg}");
}

/// Every block that declares tokens, with its selector: light, dark by system
/// preference, and dark by explicit choice. The two dark blocks must match.
fn token_blocks() -> Vec<(String, Tokens)> {
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

fn themes() -> Vec<(&'static str, Tokens)> {
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
    // A media query and an attribute cannot share a selector, so the dark values
    // are written twice; this keeps the copies from drifting.
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
    // A missing value fails: a fallback would render one theme in the other's colour.
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

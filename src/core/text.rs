//! Text handling for strings from outside (commands, paths, agent prose). Cut
//! on characters, never bytes — `&s[..80]` panics on `café.rs` — and mark every
//! cut with an ellipsis.

/// Truncates to `width` characters, the ellipsis included.
pub fn clip(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if s.chars().count() <= width {
        return s.to_string();
    }
    let mut out: String = s.chars().take(width - 1).collect();
    out.push('…');
    out
}

/// Keeps the last `n` bytes, cut on a character boundary and marked with a
/// leading ellipsis — command output's summary is at the end.
pub fn tail(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let start = s.len() - n;
    let start = (start..s.len())
        .find(|i| s.is_char_boundary(*i))
        .unwrap_or(s.len());
    format!("…\n{}", &s[start..])
}

/// Word-wraps prose to `width` for a terminal. A longer word (a path, a URL)
/// stays whole on its own line so it can still be copied.
pub fn wrap(s: &str, width: usize) -> Vec<String> {
    let width = width.max(16);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        if line.is_empty() {
            line.push_str(word);
        } else if line.chars().count() + 1 + word.chars().count() <= width {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(std::mem::take(&mut line));
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Splits YAML frontmatter from a markdown body; a file without it is all body.
/// Only the body is used as a prompt, so frontmatter directives (`allowed-tools`,
/// `model`) do not travel with it.
pub fn split_frontmatter(src: &str) -> (Option<&str>, &str) {
    let rest = match src.strip_prefix("---\n") {
        Some(r) => r,
        // Tolerate a leading blank line rather than read frontmatter as prose.
        None => match src.trim_start().strip_prefix("---\n") {
            Some(r) => r,
            None => return (None, src),
        },
    };
    match rest.split_once("\n---") {
        Some((front, after)) => (
            Some(front),
            after
                .trim_start_matches(['\r', '\n'])
                .trim_start_matches('-'),
        ),
        // An opening fence with no close is not frontmatter; treating it as one
        // would swallow the whole prompt.
        None => (None, src),
    }
}

/// Percent-encodes a query value: everything outside the unreserved set as `%XX`.
pub fn url_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_skill_contributes_its_body_and_not_its_frontmatter() {
        let (front, body) = split_frontmatter(
            "---\nname: review\nallowed-tools: Bash(git *)\n---\nReview the diff.\n",
        );
        assert!(front.unwrap().contains("allowed-tools"));
        assert_eq!(body.trim(), "Review the diff.");
    }

    #[test]
    fn a_plain_prompt_is_all_body() {
        let (front, body) = split_frontmatter("Fix the flaky login test.");
        assert!(front.is_none());
        assert_eq!(body, "Fix the flaky login test.");
    }

    #[test]
    fn an_unclosed_fence_is_not_frontmatter() {
        // Swallowing it would hand the agent an empty prompt, and it would run.
        let src = "---\nname: broken\nReview the diff.";
        let (front, body) = split_frontmatter(src);
        assert!(front.is_none());
        assert_eq!(body, src);
    }

    use super::*;

    #[test]
    fn clipping_counts_characters_not_bytes() {
        assert_eq!(clip("äöüßéè", 4), "äöü…");
        assert_eq!(clip("hello", 10), "hello");
        assert_eq!(clip("hello world", 8), "hello w…");
        assert_eq!(clip("x", 0), "");
    }

    #[test]
    fn clipping_an_emoji_does_not_panic_or_split_it() {
        let s = "run 🚀 deploy 🎉 now";
        for w in 1..=s.chars().count() + 2 {
            let out = clip(s, w);
            assert!(out.chars().count() <= w.max(1));
        }
    }

    #[test]
    fn wrapping_keeps_every_word_and_never_exceeds_the_width() {
        let s = "a rule that grants more than it looks like is worth a whole sentence";
        for w in [16, 24, 40, 76] {
            let lines = wrap(s, w);
            for l in &lines {
                assert!(l.chars().count() <= w, "{w}: {l:?}");
            }
            assert_eq!(
                lines.join(" ").split_whitespace().collect::<Vec<_>>(),
                s.split_whitespace().collect::<Vec<_>>(),
                "no word may be lost or split"
            );
        }
    }

    #[test]
    fn a_word_longer_than_the_width_is_left_whole() {
        let long = "/a/very/long/path/that/exceeds/the/width/entirely";
        let lines = wrap(&format!("see {long} now"), 20);
        assert!(lines.iter().any(|l| l == long), "{lines:?}");
    }

    #[test]
    fn wrapping_nothing_yields_one_empty_line_rather_than_none() {
        assert_eq!(wrap("", 40), vec![String::new()]);
    }

    #[test]
    fn the_tail_is_cut_on_a_character_boundary() {
        let s = "ä".repeat(10_000);
        let t = tail(&s, 1024);
        assert!(t.len() <= 1100);
        assert!(t.starts_with('…'));
        assert_eq!(tail("short", 1024), "short");
    }
}

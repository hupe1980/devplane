//! Text handling for things a human reads and an agent wrote.
//!
//! Every string on the board comes from outside: a shell command, a file path,
//! a model's prose, an issue body from the internet. Two rules follow, and both
//! were learned by getting them wrong.
//!
//! * **Cut on characters, never on bytes.** `&s[..80]` panics the moment a
//!   command contains an umlaut or an emoji, and a control plane that crashes
//!   because somebody named a file `café.rs` is not a control plane.
//! * **Say that it was cut.** A truncated line that looks complete is a line
//!   that misleads; the ellipsis is the whole point.

/// Truncates to `width` characters, marking the cut with an ellipsis.
///
/// The ellipsis is counted, so the result is never wider than asked for — which
/// is what keeps a table aligned.
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

/// Keeps the last `n` bytes of a string, cut on a character boundary and marked
/// with a leading ellipsis.
///
/// Used for command output, where the end is where the summary lives and the
/// beginning is a compilation log nobody reads.
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

/// Breaks prose onto lines no wider than `width`, for a terminal.
///
/// Word-wrapping rather than clipping, because these are *sentences*: a
/// finding that explains why a rule grants more than it looks like is useless
/// with its reason cut off, and a terminal that soft-wraps it puts the
/// continuation under the left margin where the label column is.
///
/// A word longer than `width` — a path, a URL — is left whole on its own line.
/// Breaking it would make it uncopyable, which is worse than one long line.
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

/// Splits YAML frontmatter from a markdown body.
///
/// Claude Code Skills are `---`-delimited frontmatter followed by the
/// instructions. Devplane uses the *body* as a prompt, which is portable —
/// markdown is markdown, and an agent that is not Claude reads it perfectly
/// well. What does not travel is the frontmatter's meaning: `allowed-tools`,
/// `context: fork` and `model` are directives the Claude harness honours when
/// *it* loads the skill by name, and inlining the body takes the instructions
/// without them. That is a real limitation and is why it is named here rather
/// than left for somebody to discover.
///
/// A file with no frontmatter is all body, which is the common case for a
/// hand-written prompt.
pub fn split_frontmatter(src: &str) -> (Option<&str>, &str) {
    let rest = match src.strip_prefix("---\n") {
        Some(r) => r,
        // Tolerate a leading blank line or CRLF rather than silently treating
        // the frontmatter as prose the agent should follow.
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

/// Percent-encodes a query value.
///
/// Hand-rolled rather than a dependency: the alternative is a crate for one
/// function, and the set that must be escaped in a query value is small and
/// closed. Everything outside the unreserved set goes out as `%XX`, which is
/// always correct if occasionally more than necessary.
///
/// **Here rather than beside its first caller**, because a second caller
/// arrived and copying it would have made percent-encoding a thing this
/// repository does two ways.
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
        // Claude Code's own prompt-template format. Devplane takes the body,
        // which is portable markdown any agent can follow; the frontmatter is
        // a set of directives only the Claude harness applies, and inlining it
        // as prose would read as instructions to the model.
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
        // Swallowing everything after an opening `---` would hand the agent an
        // empty prompt, which is the worst possible way to fail: it runs.
        let src = "---\nname: broken\nReview the diff.";
        let (front, body) = split_frontmatter(src);
        assert!(front.is_none());
        assert_eq!(body, src);
    }

    use super::*;

    #[test]
    fn clipping_counts_characters_not_bytes() {
        // The bug this replaces: `&s[..80]` on a command with an umlaut in it
        // panicked the receiver that was trying to describe it.
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
        // Breaking a path or a URL makes it uncopyable, which is worse than one
        // long line.
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

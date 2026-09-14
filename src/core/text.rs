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

/// Splits YAML frontmatter from a markdown body.
///
/// Claude Code Skills are `---`-delimited frontmatter followed by the
/// instructions. Vibeplane uses the *body* as a prompt, which is portable —
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

#[cfg(test)]
mod tests {
    #[test]
    fn a_skill_contributes_its_body_and_not_its_frontmatter() {
        // Claude Code's own prompt-template format. Vibeplane takes the body,
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
    fn the_tail_is_cut_on_a_character_boundary() {
        let s = "ä".repeat(10_000);
        let t = tail(&s, 1024);
        assert!(t.len() <= 1100);
        assert!(t.starts_with('…'));
        assert_eq!(tail("short", 1024), "short");
    }
}

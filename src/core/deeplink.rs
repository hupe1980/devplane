//! Links that open a coding agent.
//!
//! Claude Code registers two URL handlers with the operating system, on macOS,
//! Linux and Windows, and they are the only cross-platform way to put somebody
//! in front of a session — or to start one in the right place with the right
//! prompt. Vibeplane builds the URL and hands it over; it neither registers
//! anything nor opens anything itself.
//!
//! Two properties make this safe to generate from a supervisor, and both are
//! the provider's rather than ours:
//!
//! * **A deep link executes nothing.** It picks a directory and fills the
//!   prompt box. Claude Code shows `Prompt from an external link` until the
//!   person sends or clears it, and warns above 1 000 characters. Vibeplane
//!   never auto-sends one.
//! * **The prompt is data.** It may quote an issue body, a failing test or a
//!   tool call — text somebody else wrote, arriving at something that can run
//!   commands. It is percent-encoded here, and clipped, and that is the whole
//!   of what this module has to get right.
//!
//! What it deliberately does not do is replace `focus`. `vscode://…?session=`
//! opens in *whichever window is focused* when VS Code is already running,
//! which is worse than raising the window that actually owns the session — the
//! case this product exists for. It is the fallback when no window can be
//! identified, and the primary way to *start* something.

/// The provider's cap on a prefilled prompt. Longer is refused rather than
/// truncated by Claude Code, so the clip happens here.
pub const MAX_PROMPT: usize = 5_000;

/// Percent-encodes everything that is not unreserved.
///
/// Deliberately strict — `-_.~` and alphanumerics survive and nothing else, so
/// `&`, `#`, `?`, a newline and every non-ASCII byte are escaped. A prompt is
/// untrusted text and an under-escaped one is a query parameter somebody else
/// gets to choose.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
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

/// Clips a prompt to the provider's limit, on a character boundary.
fn prompt_param(prompt: &str) -> String {
    encode(&crate::core::text::clip(prompt, MAX_PROMPT))
}

/// Opens a Claude Code **tab in VS Code** on an existing conversation.
///
/// An unknown session id starts a fresh conversation rather than failing, and
/// a session already open is focused.
pub fn vscode_session(session_id: &str) -> String {
    format!(
        "vscode://anthropic.claude-code/open?session={}",
        encode(session_id)
    )
}

/// Opens a **new terminal session** in a local clone of a GitHub repository,
/// with the prompt typed and not sent.
///
/// `repo` is an `owner/name` slug, which Claude Code resolves to the clone or
/// worktree where `claude` last ran. It falls back to the home directory when
/// it has never seen one, so this is the right form for a link that will be
/// clicked on somebody else's machine, and the wrong one for a path only this
/// machine has.
pub fn open_repo(repo: &str, prompt: &str) -> Option<String> {
    if !is_repo_slug(repo) {
        return None;
    }
    Some(format!(
        "claude-cli://open?repo={}&q={}",
        encode(repo),
        prompt_param(prompt)
    ))
}

/// The same, for an absolute path on this machine.
///
/// Refuses what the handler refuses, so a link is never built that silently
/// does nothing: a relative path, a `..` segment, a UNC path, or a bidirectional
/// control character — the last of which can make a path read as one directory
/// and resolve as another.
pub fn open_dir(dir: &std::path::Path, prompt: &str) -> Option<String> {
    let s = dir.to_str()?;
    let rejected = !dir.is_absolute()
        || s.starts_with("\\\\")
        || dir
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        || s.chars().any(is_bidi_control);
    if rejected {
        return None;
    }
    Some(format!(
        "claude-cli://open?cwd={}&q={}",
        encode(s),
        prompt_param(prompt)
    ))
}

/// `owner/name`, and nothing that could be a second query parameter.
fn is_repo_slug(s: &str) -> bool {
    let Some((owner, name)) = s.split_once('/') else {
        return false;
    };
    let ok = |p: &str| {
        !p.is_empty()
            && p.len() <= 100
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    ok(owner) && ok(name)
}

/// The Unicode bidirectional overrides. A directory name containing one reads
/// in a different order than it resolves.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_prompt_is_data_and_is_escaped_as_data() {
        // The prompt quotes things other people wrote: an issue body, a test
        // name, a command an agent chose. Under-escaping one hands whoever
        // wrote it a second query parameter.
        let link = open_repo("acme/payments", "fix `a&b` #3\nand <c>").unwrap();
        assert!(link.starts_with("claude-cli://open?repo=acme%2Fpayments&q="));
        for bad in ['&', '#', '<', '>', '\n', '`'] {
            let after_q = link.split("&q=").nth(1).unwrap();
            assert!(!after_q.contains(bad), "{bad:?} survived in {after_q}");
        }
        // A newline is `%0A`, which is what the handler expects for a
        // multi-line prompt.
        assert!(link.contains("%0A"));
    }

    #[test]
    fn a_prompt_is_clipped_to_what_the_handler_accepts() {
        // Longer is refused by Claude Code rather than truncated, so a link
        // built from a thousand-line log would simply not work. The cap is on
        // *characters* of the prompt, not bytes of the URL — which is why this
        // asserts the decoded budget rather than the encoded length, and why
        // the clip has to happen before the encoding.
        let clipped = crate::core::text::clip(&"x".repeat(MAX_PROMPT * 2), MAX_PROMPT);
        assert!(
            clipped.chars().count() <= MAX_PROMPT,
            "{}",
            clipped.chars().count()
        );

        let link = open_repo("a/b", &"x".repeat(MAX_PROMPT * 2)).unwrap();
        let q = link.split("&q=").nth(1).unwrap();
        // `x` is unreserved, so the encoded form is the character count plus
        // whatever the ellipsis costs — a long way under twice the input.
        assert!(q.len() < MAX_PROMPT * 2, "{}", q.len());
        assert!(
            q.len() >= MAX_PROMPT - 1,
            "it should be clipped, not emptied"
        );
    }

    #[test]
    fn a_path_the_handler_would_reject_is_never_built() {
        // Building a link that silently opens the home directory is worse than
        // building none: the person clicks, something opens, and it is the
        // wrong thing.
        assert!(open_dir(Path::new("relative/path"), "x").is_none());
        assert!(open_dir(Path::new("/repo/../etc"), "x").is_none());
        assert!(open_dir(Path::new("\\\\server\\share"), "x").is_none());
        assert!(open_dir(Path::new("/repo/\u{202E}gnp.txt"), "x").is_none());
        assert!(open_dir(Path::new("/repo/app"), "x").is_some());
    }

    #[test]
    fn a_repo_slug_is_a_slug_and_not_a_query_string() {
        assert!(open_repo("acme/payments", "x").is_some());
        assert!(open_repo("acme/pay-ments.js", "x").is_some());
        assert!(open_repo("acme", "x").is_none());
        assert!(open_repo("acme/a&q=evil", "x").is_none());
        assert!(open_repo("acme/a/b", "x").is_none());
        assert!(open_repo("/name", "x").is_none());
    }

    #[test]
    fn a_session_link_names_the_session() {
        assert_eq!(
            vscode_session("abc-123"),
            "vscode://anthropic.claude-code/open?session=abc-123"
        );
    }
}

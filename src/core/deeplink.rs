//! Claude Code deep links, which Devplane builds but never opens. A link
//! executes nothing — it fills the prompt box and Claude Code marks it as from
//! an external link — but the prompt is untrusted text, so it is strictly
//! percent-encoded and clipped. `vscode://…?session=` opens in whichever window
//! is focused, so it is a fallback to `focus`, not a replacement.

/// Claude Code refuses a longer prefilled prompt rather than truncating it.
pub const MAX_PROMPT: usize = 5_000;

/// Strict: only `-_.~` and alphanumerics survive, so an untrusted prompt cannot
/// smuggle in a query parameter.
use crate::core::text::url_escape as encode;

/// Clips a prompt to the provider's limit, on a character boundary.
fn prompt_param(prompt: &str) -> String {
    encode(&crate::core::text::clip(prompt, MAX_PROMPT))
}

/// Opens a new terminal session in a local clone of `repo` (an `owner/name`
/// slug), with the prompt typed and not sent.
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

/// A session id safe to put on a command line and in a link: what Claude Code
/// issues (a UUID) and nothing that could become a second argument.
fn is_session_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// The command that reaches a Claude Code session from a terminal: `claude
/// --resume <id>`, or `claude attach <id>` for one Claude Code's own daemon
/// runs in the background — the command `devplane run attach` runs.
pub fn resume_command(session: &str, background: bool) -> Option<String> {
    if !is_session_id(session) {
        return None;
    }
    Some(match background {
        true => format!("claude attach {session}"),
        false => format!("claude --resume {session}"),
    })
}

/// The VS Code extension's documented link to a session, which focuses its
/// tab where it is open — offered only for a session the extension started.
pub fn session_link(session: &str, entrypoint: Option<&str>) -> Option<String> {
    if entrypoint != Some("claude-vscode") || !is_session_id(session) {
        return None;
    }
    Some(format!(
        "vscode://anthropic.claude-code/open?session={}",
        encode(session)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_is_reached_by_its_own_command_or_documented_link() {
        assert_eq!(
            resume_command("0b9e-41", false).as_deref(),
            Some("claude --resume 0b9e-41")
        );
        assert_eq!(
            resume_command("0b9e-41", true).as_deref(),
            Some("claude attach 0b9e-41")
        );
        assert_eq!(resume_command("x; rm -rf ~", false), None);
        assert_eq!(resume_command("", false), None);
        assert_eq!(
            session_link("0b9e-41", Some("claude-vscode")).as_deref(),
            Some("vscode://anthropic.claude-code/open?session=0b9e-41")
        );
        assert_eq!(session_link("0b9e-41", Some("cli")), None);
        assert_eq!(session_link("0b9e-41", None), None);
    }

    #[test]
    fn a_prompt_is_data_and_is_escaped_as_data() {
        let link = open_repo("acme/payments", "fix `a&b` #3\nand <c>").unwrap();
        assert!(link.starts_with("claude-cli://open?repo=acme%2Fpayments&q="));
        for bad in ['&', '#', '<', '>', '\n', '`'] {
            let after_q = link.split("&q=").nth(1).unwrap();
            assert!(!after_q.contains(bad), "{bad:?} survived in {after_q}");
        }
        assert!(link.contains("%0A"));
    }

    #[test]
    fn a_prompt_is_clipped_to_what_the_handler_accepts() {
        // The cap is on prompt characters, so the clip precedes encoding.
        let clipped = crate::core::text::clip(&"x".repeat(MAX_PROMPT * 2), MAX_PROMPT);
        assert!(
            clipped.chars().count() <= MAX_PROMPT,
            "{}",
            clipped.chars().count()
        );

        let link = open_repo("a/b", &"x".repeat(MAX_PROMPT * 2)).unwrap();
        let q = link.split("&q=").nth(1).unwrap();
        // `x` is unreserved: encoded length is the characters plus the ellipsis.
        assert!(q.len() < MAX_PROMPT * 2, "{}", q.len());
        assert!(
            q.len() >= MAX_PROMPT - 1,
            "it should be clipped, not emptied"
        );
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
}

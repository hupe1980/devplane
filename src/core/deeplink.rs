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

#[cfg(test)]
mod tests {
    use super::*;

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

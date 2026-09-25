//! Which provider this machine uses. Per Claude Code's availability matrix,
//! what runs an agent (hooks, OpenTelemetry, skills) works everywhere, while
//! the vendor's supervision surfaces need a claude.ai sign-in — so on Bedrock,
//! Vertex, Foundry or a gateway, Devplane is the only gate. Pure over a
//! captured environment.

/// How this machine authenticates, which is what decides the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// A claude.ai sign-in: the vendor's full supervision layer is available.
    Subscription,
    Console,
    Bedrock,
    Vertex,
    Foundry,
    /// Some other base URL: a corporate LLM gateway or a proxy.
    Gateway,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Subscription => "claude.ai subscription",
            Provider::Console => "Anthropic Console API key",
            Provider::Bedrock => "Amazon Bedrock",
            Provider::Vertex => "Google Cloud's Agent Platform",
            Provider::Foundry => "Microsoft Foundry",
            Provider::Gateway => "an LLM gateway",
        }
    }

    /// The variable that decided it, so a person can check why.
    pub fn because(&self) -> &'static str {
        match self {
            Provider::Subscription => "no provider variable is set",
            Provider::Console => "ANTHROPIC_API_KEY is set",
            Provider::Bedrock => "CLAUDE_CODE_USE_BEDROCK is set",
            Provider::Vertex => "CLAUDE_CODE_USE_VERTEX is set",
            Provider::Foundry => "CLAUDE_CODE_USE_FOUNDRY is set",
            Provider::Gateway => "ANTHROPIC_BASE_URL points away from api.anthropic.com",
        }
    }

    pub fn has_vendor_supervision(&self) -> bool {
        *self == Provider::Subscription
    }

    /// What a claude.ai sign-in buys that this provider lacks, per the vendor's matrix.
    pub fn missing(&self) -> &'static [&'static str] {
        if self.has_vendor_supervision() {
            return &[];
        }
        &[
            "Remote Control",
            "Routines (/schedule)",
            "ultrareview",
            "Code Review",
            "Channels",
            "Claude Code on the web, mobile and Desktop",
            "the analytics dashboard and API",
            "server-managed settings",
        ]
    }

    /// What still works: exactly Devplane's substrate.
    pub fn intact(&self) -> &'static [&'static str] {
        &[
            "hooks",
            "OpenTelemetry metrics",
            "workflows",
            "skills and commands",
            "subagents",
            "sandboxing",
            "MCP servers",
            "the managed settings file",
        ]
    }

    /// Surfaces present but reduced.
    pub fn partial(&self) -> &'static [&'static str] {
        if self.has_vendor_supervision() {
            return &[];
        }
        &["auto mode — fewer models, and sessions start in Manual"]
    }
}

/// Reads the provider out of an environment, in the vendor's credential
/// precedence.
pub fn detect(get: impl Fn(&str) -> Option<String>) -> Provider {
    let set = |k: &str| {
        get(k)
            .map(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(false)
    };
    if set("CLAUDE_CODE_USE_BEDROCK") {
        return Provider::Bedrock;
    }
    if set("CLAUDE_CODE_USE_VERTEX") {
        return Provider::Vertex;
    }
    if set("CLAUDE_CODE_USE_FOUNDRY") {
        return Provider::Foundry;
    }
    if let Some(url) = get("ANTHROPIC_BASE_URL")
        && !url.is_empty()
        && !url.contains("api.anthropic.com")
    {
        return Provider::Gateway;
    }
    if set("ANTHROPIC_API_KEY") || set("ANTHROPIC_AUTH_TOKEN") {
        return Provider::Console;
    }
    Provider::Subscription
}

pub fn from_env() -> Provider {
    detect(|k| std::env::var(k).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn a_machine_with_nothing_set_is_the_subscription_case() {
        assert_eq!(detect(env(&[])), Provider::Subscription);
        assert!(detect(env(&[])).has_vendor_supervision());
        assert!(detect(env(&[])).missing().is_empty());
    }

    #[test]
    fn each_provider_variable_is_recognised_in_the_vendors_own_order() {
        assert_eq!(
            detect(env(&[("CLAUDE_CODE_USE_BEDROCK", "1")])),
            Provider::Bedrock
        );
        assert_eq!(
            detect(env(&[("CLAUDE_CODE_USE_VERTEX", "true")])),
            Provider::Vertex
        );
        assert_eq!(
            detect(env(&[("CLAUDE_CODE_USE_FOUNDRY", "1")])),
            Provider::Foundry
        );
        // A cloud provider outranks a key.
        assert_eq!(
            detect(env(&[
                ("ANTHROPIC_API_KEY", "sk-x"),
                ("CLAUDE_CODE_USE_BEDROCK", "1"),
            ])),
            Provider::Bedrock
        );
    }

    #[test]
    fn a_variable_set_to_nothing_is_a_variable_that_is_not_set() {
        // Exported-and-empty is how a profile leaves a variable meant unset.
        for off in ["", "0", "false", "FALSE"] {
            assert_eq!(
                detect(env(&[("CLAUDE_CODE_USE_BEDROCK", off)])),
                Provider::Subscription,
                "{off:?} is not on"
            );
        }
    }

    #[test]
    fn a_base_url_decides_only_when_it_points_away() {
        assert_eq!(
            detect(env(&[("ANTHROPIC_BASE_URL", "https://llm.corp.example")])),
            Provider::Gateway
        );
        // Pointing at the vendor is not a gateway, and neither is an empty one.
        assert_eq!(
            detect(env(&[("ANTHROPIC_BASE_URL", "https://api.anthropic.com")])),
            Provider::Subscription
        );
        assert_eq!(
            detect(env(&[("ANTHROPIC_BASE_URL", "")])),
            Provider::Subscription
        );
    }

    #[test]
    fn every_provider_that_loses_supervision_keeps_devplanes_substrate() {
        for p in [
            Provider::Bedrock,
            Provider::Vertex,
            Provider::Foundry,
            Provider::Gateway,
            Provider::Console,
        ] {
            assert!(!p.has_vendor_supervision(), "{p:?}");
            assert!(!p.missing().is_empty(), "{p:?}");
            assert!(p.intact().contains(&"hooks"), "{p:?}");
            assert!(p.intact().contains(&"OpenTelemetry metrics"), "{p:?}");
        }
    }
}

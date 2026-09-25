//! What is watched here, per vendor, per channel.
//!
//! Driving (ACP) is identical for every agent; watching a session somebody
//! opened in their own terminal depends on what each vendor publishes. Each row
//! is a fact about another product and carries the date it was last checked
//! against that vendor's documentation. `devplane doctor` prints this table.

use serde::{Deserialize, Serialize};

/// A way a session can tell Devplane something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Lifecycle and tool calls, and the only one that can refuse a call.
    Hooks,
    /// Per-request cost and tokens, over OpenTelemetry.
    Telemetry,
    /// What the vendor's own background process supervises — the only channel
    /// that can discover a session that has never sent an event.
    Roster,
    /// Rate limits and the model, from the status line.
    StatusLine,
    /// The vendor's own event stream, which Devplane connects out to; nothing is
    /// installed into the agent.
    EventFeed,
}

impl Channel {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Hooks => "hooks",
            Channel::Telemetry => "telemetry",
            Channel::Roster => "roster",
            Channel::StatusLine => "status line",
            Channel::EventFeed => "event feed",
        }
    }

    /// What is lost when this channel is absent, in surface terms.
    #[must_use]
    pub fn costs(self) -> &'static str {
        match self {
            Channel::Hooks => "no tool calls, no permissions, and nothing can be refused",
            Channel::Telemetry => "no cost and no token counts",
            Channel::Roster => "a session that has never sent an event cannot be discovered",
            Channel::StatusLine => "no rate limit and no model name",
            Channel::EventFeed => {
                "no questions, no permissions and no session states from a vendor that publishes them"
            }
        }
    }
}

/// How well Devplane can read one channel of one vendor. *The vendor offers
/// nothing*, *we have not built it* and *nobody looked* are distinct claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    /// Devplane reads it, and it has been demonstrated end to end.
    Read,
    /// The vendor publishes it and Devplane reads it; nobody has run it.
    Unproved,
    /// The vendor publishes it and Devplane does not read it yet.
    Unbuilt,
    /// The vendor publishes nothing Devplane could read here.
    NotPublished,
    /// Nobody has read this vendor's documentation for this channel.
    Unchecked,
}

impl Reach {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Reach::Read => "read",
            Reach::Unproved => "unproved",
            Reach::Unbuilt => "unbuilt",
            Reach::NotPublished => "not published",
            Reach::Unchecked => "not checked",
        }
    }

    /// Whether a surface may render data from this channel as a fact.
    /// `Unproved` may (the path exists); `Unbuilt` may not — there is no code
    /// path, and true would put the vendor on the *unproved* list.
    #[must_use]
    pub fn may_render(self) -> bool {
        !matches!(
            self,
            Reach::NotPublished | Reach::Unbuilt | Reach::Unchecked
        )
    }
}

/// One vendor's one channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub vendor: &'static str,
    pub channel: Channel,
    pub reach: Reach,
    /// Why it reads that way, in one sentence a person can act on.
    pub because: &'static str,
    /// When this row was last read against the vendor's own documentation.
    pub checked: &'static str,
}

/// Whether this vendor's channels can show that a question was abandoned.
/// Lives here so no surface branches on vendor identity. Claude Code derives it
/// from hook events; Copilot documents the events but nobody has run it;
/// OpenCode names the ending itself (`question.rejected`), also never run.
#[must_use]
pub fn can_show_an_abandoned_question(vendor: &str) -> Reach {
    match vendor {
        "Claude Code" => Reach::Read,
        "GitHub Copilot" => Reach::Unproved,
        "OpenCode" => Reach::Unproved,
        _ => Reach::NotPublished,
    }
}

/// [`can_show_an_abandoned_question`] for an agent id rather than a vendor name.
#[must_use]
pub fn agent_can_show_an_abandoned_question(agent: &str) -> Reach {
    match vendor_of(agent) {
        Some(v) => can_show_an_abandoned_question(v),
        None => Reach::NotPublished,
    }
}

/// The vendor an agent id belongs to, as this table spells it — the only such
/// mapping.
#[must_use]
pub fn vendor_of(agent: &str) -> Option<&'static str> {
    match agent.to_ascii_lowercase().as_str() {
        "claude" | "claude-code" => Some("Claude Code"),
        "copilot" | "github-copilot" => Some("GitHub Copilot"),
        "opencode" => Some("OpenCode"),
        _ => None,
    }
}

/// The oldest row's `checked` date — the table's weakest claim. A test holds it
/// to the minimum.
pub const CHECKED: &str = "2026-09-21";

/// What is watched here. Driving is deliberately absent: it is identical for
/// every ACP agent and would invite averaging the two into one impression.
#[must_use]
pub fn watched() -> Vec<Row> {
    use Channel::*;
    use Reach::*;
    vec![
        Row {
            vendor: "Claude Code",
            channel: Hooks,
            reach: Read,
            because: "lifecycle, tool calls and the permission gate, all documented",
            checked: "2026-09-24",
        },
        Row {
            vendor: "Claude Code",
            channel: Telemetry,
            reach: Read,
            because: "OpenTelemetry export, per request",
            checked: "2026-09-24",
        },
        Row {
            vendor: "Claude Code",
            channel: Roster,
            reach: Read,
            because: "the background roster names what the vendor's own daemon supervises",
            checked: "2026-09-21",
        },
        Row {
            vendor: "Claude Code",
            channel: StatusLine,
            reach: Read,
            because: "the optional shim, for rate limits and the model",
            checked: "2026-09-21",
        },
        Row {
            vendor: "GitHub Copilot",
            channel: Hooks,
            reach: Unproved,
            // No count: `observe::copilot::EVENTS` is the ledger.
            because: "every documented event is in one ledger, mapped or silent by decision, \
                        and every hook runs the binary. Nobody has yet run a session with them \
                        installed",
            checked: "2026-09-24",
        },
        Row {
            vendor: "GitHub Copilot",
            channel: Telemetry,
            reach: Unproved,
            because: "COPILOT_OTEL_ENABLED and an OTLP endpoint, read as a dialect of the same \
                        exporter. Never measured against a running session",
            checked: "2026-09-21",
        },
        Row {
            vendor: "GitHub Copilot",
            channel: Roster,
            reach: NotPublished,
            because: "~/.copilot holds flat process logs and no session roster, so a Copilot \
                        session that has sent no hook cannot be discovered at all",
            checked: "2026-09-21",
        },
        Row {
            vendor: "GitHub Copilot",
            channel: StatusLine,
            reach: NotPublished,
            because: "no status-line protocol; usage is AI credits rather than dollars and is \
                        not published per session",
            checked: "2026-09-21",
        },
        Row {
            vendor: "Codex",
            channel: Hooks,
            reach: Unproved,
            because: "twelve documented events in Claude Code's own payload and answer shapes, \
                      so the same shim serves; `connect codex` merges the entries into its \
                      hooks.json, and Codex runs none until a person approves them in its \
                      dialog. Nobody has yet run a session with them installed",
            checked: "2026-09-25",
        },
        Row {
            vendor: "Codex",
            channel: Telemetry,
            reach: Unchecked,
            because: "no fetched reference for Codex telemetry exists here, so whether it \
                      exports anything an OTLP receiver could read is not a claim this table \
                      can make",
            checked: "2026-09-24",
        },
        Row {
            vendor: "OpenCode",
            channel: Hooks,
            reach: NotPublished,
            because: "the vendor's documented surface for this is its HTTP event feed; no hook \
                      protocol appears in its API",
            checked: "2026-09-23",
        },
        Row {
            vendor: "OpenCode",
            channel: EventFeed,
            reach: Unproved,
            because: "GET /event carries question.asked, question.replied and question.rejected — \
                      a question nobody answered, named by the vendor rather than derived — and \
                      the permission family beside them. Measured against opencode serve \
                      1.18.30 on 2026-09-23; question.rejected is closed over sessionID and \
                      requestID, so the authority on an ending is nobody and the vendor says so",
            checked: "2026-09-24",
        },
        Row {
            vendor: "OpenCode",
            channel: Roster,
            reach: Unproved,
            because: "GET /session is documented and read on every connect, so a session \
                      Devplane never started is listed before it emits anything. Nobody has \
                      run it against a live server",
            checked: "2026-09-24",
        },
        Row {
            vendor: "Gemini CLI",
            channel: Hooks,
            reach: Unchecked,
            because: "no fetched reference for this vendor exists here, so whether it publishes \
                      anything watchable is not a claim this table can make",
            checked: "2026-09-21",
        },
    ]
}

/// In table order.
#[must_use]
pub fn vendors() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for r in watched() {
        if !out.contains(&r.vendor) {
            out.push(r.vendor);
        }
    }
    out
}

/// What a surface says instead of an empty list: empty and unseeable are
/// opposite facts, and a board with no rows must not read as *nothing is
/// happening* when it is the limit of its sight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watching {
    /// Watched, and demonstrated end to end.
    pub watched: Vec<String>,
    /// Channels are read and nobody has run them.
    pub unproved: Vec<String>,
    /// Visible only when Devplane starts them; no surface may imply otherwise.
    pub driven_only: Vec<String>,
}

#[must_use]
pub fn watching() -> Watching {
    let rows = watched();
    let mut out = Watching {
        watched: Vec::new(),
        unproved: Vec::new(),
        driven_only: Vec::new(),
    };
    for vendor in vendors() {
        let mine = || rows.iter().filter(|r| r.vendor == vendor);
        // The best reach of any channel: one working channel makes a session
        // appear.
        if mine().any(|r| r.reach == Reach::Read) {
            out.watched.push(vendor.to_string());
        } else if mine().any(|r| r.reach.may_render()) {
            out.unproved.push(vendor.to_string());
        } else {
            out.driven_only.push(vendor.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_agent_that_can_be_driven_has_a_row_saying_whether_it_can_be_watched() {
        for spec in crate::acp::builtin() {
            let named = vendors().iter().any(|v| {
                let v = v.to_lowercase();
                v.contains(&spec.id) || spec.name.to_lowercase().contains(&v)
            });
            assert!(
                named,
                "`{}` can be driven and this table does not say whether it can be watched. \
                 Driving and watching are different lists, and a vendor missing from this one \
                 is how the second list quietly inherits the first one's length.",
                spec.id
            );
        }
    }

    #[test]
    fn every_row_says_why() {
        for r in watched() {
            assert!(
                r.because.len() > 20,
                "{} / {} reads `{}` for no stated reason",
                r.vendor,
                r.channel.as_str(),
                r.reach.as_str()
            );
        }
    }

    #[test]
    fn a_vendor_that_cannot_be_watched_is_never_reported_as_quiet() {
        let w = watching();
        assert!(w.watched.iter().any(|x| x == "Claude Code"));
        assert!(
            w.unproved.iter().any(|x| x == "GitHub Copilot"),
            "unproved is not watched and not absent"
        );
        assert!(!w.watched.iter().any(|x| x == "GitHub Copilot"));
        assert!(
            w.driven_only.iter().any(|x| x == "Gemini CLI"),
            "Gemini CLI can only be seen when Devplane starts it, and no list says so"
        );
        assert!(w.unproved.iter().any(|x| x == "Codex"));
        assert!(!w.watched.iter().any(|x| x == "Codex"));
        // Pinned by name: a vendor changing list is what this table records.
        assert!(
            w.unproved.iter().any(|x| x == "OpenCode"),
            "OpenCode's feed is read and unproved, and no list says so"
        );
        assert!(
            !w.watched.iter().any(|x| x == "OpenCode"),
            "a channel nobody has run is being promised as watched"
        );
        assert!(!w.driven_only.iter().any(|x| x == "OpenCode"));
        let total = w.watched.len() + w.unproved.len() + w.driven_only.len();
        assert_eq!(total, vendors().len(), "a vendor is on two lists or none");
    }

    /// A `NotPublished` reason that talks about building is an `Unbuilt` row
    /// wearing the wrong value.
    #[test]
    fn a_not_published_row_states_the_vendors_position_and_not_our_backlog() {
        const OURS: [&str; 6] = [
            "has been built",
            "been built",
            "not built",
            "unbuilt",
            "not implemented",
            "devplane",
        ];
        let mut checked = 0;
        for r in watched().iter().filter(|r| r.reach == Reach::NotPublished) {
            checked += 1;
            let because = r.because.to_ascii_lowercase();
            for phrase in OURS {
                assert!(
                    !because.contains(phrase),
                    "{} / {} is `not published` — a claim that the vendor offers nothing — and its \
                     reason says \"{}\", which is about us. Use `Unbuilt` where the vendor \
                     publishes it and we have not read it, or `Unchecked` where nobody has looked",
                    r.vendor,
                    r.channel.as_str(),
                    r.because
                );
            }
        }
        assert!(
            checked >= 3,
            "only {checked} rows are `not published`, so this check has stopped seeing the table"
        );
    }

    #[test]
    fn a_published_channel_nobody_built_is_not_a_channel_anything_renders() {
        assert!(!Reach::Unbuilt.may_render());
        assert!(!Reach::Unchecked.may_render());
        assert!(Reach::Unproved.may_render(), "unproved still renders");

        // A vendor with *nothing* renderable is on no optimistic list.
        let all_rows = watched();
        let nothing_built: Vec<&str> = all_rows
            .iter()
            .map(|r| r.vendor)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|v| {
                all_rows
                    .iter()
                    .filter(|r| r.vendor == *v)
                    .all(|r| !r.reach.may_render())
            })
            .collect();
        assert!(
            !nothing_built.is_empty(),
            "every vendor has something built, so this check is inert"
        );
        let w = watching();
        for v in nothing_built {
            assert!(
                !w.watched.iter().any(|x| x == v) && !w.unproved.iter().any(|x| x == v),
                "`{v}` has nothing built at all and is being reported as watched"
            );
        }
    }

    #[test]
    fn every_row_carries_its_date_and_the_headline_is_the_oldest() {
        let oldest = watched().iter().map(|r| r.checked).min().expect("rows");
        assert_eq!(
            CHECKED, oldest,
            "`CHECKED` is not the oldest row date; a row was re-read and the headline not moved, \
             or the headline was moved past a row that was not re-read"
        );
        for r in watched() {
            assert!(
                r.checked.len() == 10 && r.checked.split('-').count() == 3,
                "{} / {} carries `{}`, which is not a date",
                r.vendor,
                r.channel.as_str(),
                r.checked
            );
        }
    }

    /// `observe::opencode` reads the roster and `question.rejected`.
    #[test]
    fn opencode_rows_match_what_the_feed_reader_reads() {
        let roster = watched()
            .into_iter()
            .find(|r| r.vendor == "OpenCode" && r.channel == Channel::Roster)
            .expect("an OpenCode roster row");
        assert_eq!(roster.reach, Reach::Unproved);
        assert_eq!(can_show_an_abandoned_question("OpenCode"), Reach::Unproved);
        assert!(
            can_show_an_abandoned_question("OpenCode").may_render(),
            "a vendor whose ending is read may not be told it cannot be shown"
        );
    }

    /// Colour may never be the only thing carrying a distinction.
    #[test]
    fn every_reach_has_its_own_word() {
        let words = [
            Reach::Read,
            Reach::Unproved,
            Reach::Unbuilt,
            Reach::NotPublished,
            Reach::Unchecked,
        ]
        .map(Reach::as_str);
        let unique: std::collections::BTreeSet<&str> = words.iter().copied().collect();
        assert_eq!(
            unique.len(),
            words.len(),
            "two reaches print the same word: {words:?}"
        );
    }
}

//! What is watched here, per vendor, per channel.
//!
//! **Because *watching* and *driving* are different lists and only one of them
//! is five long.** An agent Devplane starts reports through the Agent Client
//! Protocol by construction — that is the easy half, and it is the same for
//! every agent that speaks it. The claim this product actually makes is about
//! **the session somebody opened in their own terminal**, and that one is
//! answered channel by channel by whatever the vendor publishes.
//!
//! This table exists so the answer can be read off a command rather than out of
//! this source file. A person choosing a tool on the strength of *every agent
//! on your machine* should be able to find out what that covers before they
//! install it, not after.
//!
//! **Every row is a fact about somebody else's product and decays on their
//! schedule.** Each carries the date it was last checked against the vendor's
//! own documentation, because a row nobody re-reads is how this table came to
//! say Copilot had no permission event for eleven passes while its hooks
//! reference documented one.

use serde::{Deserialize, Serialize};

/// A way a session can tell Devplane something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Lifecycle and tool calls, and the only one that can refuse a call.
    Hooks,
    /// Per-request cost and tokens, over OpenTelemetry.
    Telemetry,
    /// What the vendor's own daemon supervises — the only channel that can
    /// discover a session Devplane has never received an event for.
    Roster,
    /// Rate limits and the model, from the status line.
    StatusLine,
    /// The vendor's own event stream, **subscribed to rather than pushed**.
    ///
    /// Every other channel is the vendor's process telling Devplane something;
    /// this is a server the person already runs, which Devplane connects out to.
    /// Nothing is installed into the agent, and a session Devplane did not start
    /// is visible.
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

    /// What is lost when this channel is absent — in terms of the surface a
    /// person is looking at, never in terms of the protocol.
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

/// How well Devplane can read one channel of one vendor.
///
/// **Four states, and the two middle ones are the ones that matter.** *Read* and
/// *absent* are the easy answers. **Built but never demonstrated** is the honest
/// description of most cross-vendor work at any given moment, and collapsing it
/// into *read* is how a claim becomes half true without anybody lying.
///
/// **`Unbuilt` and `Unchecked` exist because `NotPublished` was carrying them.**
/// One value said *the vendor offers nothing* while meaning *we have not built
/// it* or *nobody looked* — which made OpenCode's row false in the reassuring
/// direction. *Nobody can* and *nobody has yet* are the only question a roadmap
/// asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    /// Devplane reads it, and it has been demonstrated end to end.
    Read,
    /// The vendor publishes it and Devplane reads it; **nobody has run it**.
    Unproved,
    /// The vendor publishes it and **Devplane does not read it yet**.
    Unbuilt,
    /// The vendor publishes nothing Devplane could read here.
    NotPublished,
    /// **Nobody has read this vendor's documentation for this channel.**
    ///
    /// The third thing `NotPublished` was being used for. *The vendor has none*
    /// and *nobody looked* are not the same claim, and only one of them is safe
    /// to make without a fetched reference — which is this table's own rule and
    /// the constitution's third principle.
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
    ///
    /// `Unproved` answers **true**: the path exists and may well work, and
    /// refusing to render it would be a different lie. What must not happen is
    /// a surface reporting *nothing here* when the truth is *nothing readable
    /// here* — which is what [`Reach::NotPublished`] is for.
    ///
    /// **`Unbuilt` answers false.** The vendor publishes it, so it is not
    /// absent; Devplane reads nothing, so there is nothing to render. True here
    /// would put the vendor on the *unproved* list, which promises a code path
    /// that does not exist. *Cannot* versus *have not* lives in
    /// `devplane doctor`, not in every empty state.
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
}

/// Whether this vendor's channels can show that a question was **abandoned**.
///
/// **A row, not an `if`.** This was `agent != "claude"` inside an API handler:
/// a vendor identity compared against a string literal, in the one product
/// whose argument is that a vendor is a row in a table with a date on it. The
/// answer is a property of what a vendor *documents*, so it belongs beside the
/// channels it documents.
///
/// The derivation is a subtraction over hook events — a question asked, and the
/// run moving past it without the tool completing. Claude Code documents the
/// events it needs. Copilot documents `permissionRequest`, `notification` and a
/// `sessionEnd` reason, which is why its answer is **unproved** rather than no:
/// nobody has run it.
#[must_use]
pub fn can_show_an_abandoned_question(vendor: &str) -> Reach {
    match vendor {
        "Claude Code" => Reach::Read,
        "GitHub Copilot" => Reach::Unproved,
        // `question.rejected` is documented; Claude Code's equivalent must be
        // derived by subtracting two hooks.
        "OpenCode" => Reach::Unbuilt,
        _ => Reach::NotPublished,
    }
}

/// The same question asked of an **agent id** — what a run carries — rather
/// than of a vendor's display name.
///
/// Two spellings of one fact is how the string comparison got there in the
/// first place, so the mapping is here and the rule is above it.
#[must_use]
pub fn agent_can_show_an_abandoned_question(agent: &str) -> Reach {
    match vendor_of(agent) {
        Some(v) => can_show_an_abandoned_question(v),
        None => Reach::NotPublished,
    }
}

/// The vendor an agent id belongs to, as this table spells it.
///
/// One mapping, because two spellings of one fact is how a vendor comparison
/// gets written into a surface.
#[must_use]
pub fn vendor_of(agent: &str) -> Option<&'static str> {
    match agent.to_ascii_lowercase().as_str() {
        "claude" | "claude-code" => Some("Claude Code"),
        "copilot" | "github-copilot" => Some("GitHub Copilot"),
        "opencode" => Some("OpenCode"),
        _ => None,
    }
}

/// The date every row below was last checked against a vendor's own
/// documentation.
pub const CHECKED: &str = "2026-09-21";

/// What is watched here.
///
/// **Driving is deliberately absent from this table.** Every agent that speaks
/// the Agent Client Protocol is driven identically, so a column for it would be
/// five identical ticks and would invite the reader to average the two halves
/// into one impression — which is the exact misreading this table exists to
/// prevent.
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
        },
        Row {
            vendor: "Claude Code",
            channel: Telemetry,
            reach: Read,
            because: "OpenTelemetry export, per request",
        },
        Row {
            vendor: "Claude Code",
            channel: Roster,
            reach: Read,
            because: "the background roster names what the vendor's own daemon supervises",
        },
        Row {
            vendor: "Claude Code",
            channel: StatusLine,
            reach: Read,
            because: "the optional shim, for rate limits and the model",
        },
        Row {
            vendor: "GitHub Copilot",
            channel: Hooks,
            reach: Unproved,
            because: "fourteen documented events; twelve are mapped and two are silent by \
                        decision. Nobody has yet run a session with them installed",
        },
        Row {
            vendor: "GitHub Copilot",
            channel: Telemetry,
            reach: Unproved,
            because: "COPILOT_OTEL_ENABLED and an OTLP endpoint, read as a dialect of the same \
                        exporter. Never measured against a running session",
        },
        Row {
            vendor: "GitHub Copilot",
            channel: Roster,
            reach: NotPublished,
            because: "~/.copilot holds flat process logs and no session roster, so a Copilot \
                        session that has sent no hook cannot be discovered at all",
        },
        Row {
            vendor: "GitHub Copilot",
            channel: StatusLine,
            reach: NotPublished,
            because: "no status-line protocol; usage is AI credits rather than dollars and is \
                        not published per session",
        },
        // One row each, because the answer is the same for all three and
        // repeating it four times would suggest somebody checked four things.
        Row {
            vendor: "Codex",
            channel: Hooks,
            reach: NotPublished,
            because: "its documented surface is an app-server protocol a client drives, not a \
                      stream a watcher subscribes to",
        },
        // Three rows, because three separate facts were being averaged into one.
        Row {
            vendor: "OpenCode",
            channel: Hooks,
            reach: NotPublished,
            because: "the vendor's documented surface for this is its HTTP event feed; no hook \
                      protocol appears in its API",
        },
        Row {
            vendor: "OpenCode",
            channel: EventFeed,
            reach: Unbuilt,
            because: "GET /event is a documented text/event-stream carrying question.asked, \
                      question.replied and question.rejected — a question nobody answered, named \
                      by the vendor rather than derived",
        },
        Row {
            vendor: "OpenCode",
            channel: Roster,
            reach: Unbuilt,
            because: "GET /session is documented and lists sessions the server supervises, so a \
                      session Devplane never started is discoverable",
        },
        Row {
            vendor: "Gemini CLI",
            channel: Hooks,
            reach: Unchecked,
            because: "no fetched reference for this vendor exists here, so whether it publishes \
                      anything watchable is not a claim this table can make",
        },
    ]
}

/// The vendors this table speaks for, in the order it lists them.
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

/// What a surface says **instead of** an empty list.
///
/// **Empty and unseeable are opposite facts, and the reassuring one is the
/// wrong default.** A board with no rows on a machine running three Codex
/// sessions is not reporting quiet; it is reporting the limit of its own sight,
/// and a person reading it as *nothing is happening* has been misled by an
/// interface that was technically correct.
///
/// The `ls` command already got this right — it says *no **Claude Code**
/// sessions are running* rather than *no sessions* — and the board did not,
/// which is how one surface can be honest and its twin can not be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Watching {
    /// Watched, and demonstrated end to end.
    pub watched: Vec<&'static str>,
    /// Channels are read and nobody has run them. Named separately because
    /// promising these would be the same overstatement in a smaller font.
    pub unproved: Vec<&'static str>,
    /// Visible only when Devplane starts them. A session you opened yourself in
    /// one of these does not appear, and no surface may imply otherwise.
    pub driven_only: Vec<&'static str>,
}

/// Who is on which list.
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
        // **The best reach any of its channels has**, because one working
        // channel is the difference between a session appearing and not. A
        // vendor with hooks but no roster is watched, with less detail.
        if mine().any(|r| r.reach == Reach::Read) {
            out.watched.push(vendor);
        } else if mine().any(|r| r.reach.may_render()) {
            out.unproved.push(vendor);
        } else {
            out.driven_only.push(vendor);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every vendor Devplane can drive is named here.**
    ///
    /// The failure this catches is the one the README made: five agents in the
    /// driving list, and a sentence next to it implying the watching list was
    /// the same five. A vendor added to `acp::builtin()` and not to this table
    /// is a vendor whose watching story nobody wrote down.
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

    /// A row that says nothing is worse than no row: it looks like an answer.
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

    /// **A vendor that cannot be seen is on a different list from a quiet
    /// one.**
    ///
    /// This is the predicate a surface asks instead of rendering an empty list,
    /// and getting it backwards is the reassuring failure: *nothing is waiting*
    /// where the truth is *nothing can be seen*.
    #[test]
    fn a_vendor_that_cannot_be_watched_is_never_reported_as_quiet() {
        let w = watching();
        assert!(w.watched.contains(&"Claude Code"));
        // Unproved is its own list: promising it would be the same
        // overstatement in a smaller font.
        assert!(
            w.unproved.contains(&"GitHub Copilot"),
            "unproved is not watched and not absent"
        );
        assert!(!w.watched.contains(&"GitHub Copilot"));
        for v in ["Codex", "OpenCode", "Gemini CLI"] {
            assert!(
                w.driven_only.contains(&v),
                "`{v}` can only be seen when Devplane starts it, and no list says so"
            );
        }
        // Every vendor is on exactly one list, or a surface can print the same
        // one twice and say two different things about it.
        let total = w.watched.len() + w.unproved.len() + w.driven_only.len();
        assert_eq!(total, vendors().len(), "a vendor is on two lists or none");
    }

    /// **A reach is a claim about the vendor; a reason may not smuggle in a
    /// claim about us.**
    ///
    /// `NotPublished` is defined here as *the vendor publishes nothing Devplane
    /// could read* — and three rows carried it with the reason *"driven over the
    /// protocol only; no observation channel has been built"*, which is a fact
    /// about **Devplane's** backlog. For OpenCode the row was outright false in
    /// the reassuring direction: the vendor documents a session list and an event
    /// stream naming `question.rejected`, which is more than the one vendor this
    /// product does read, where the same fact must be derived by subtracting two
    /// hooks. `devplane doctor` printed it.
    ///
    /// One value was doing three jobs — *the vendor has none*, *we have not built
    /// it*, *nobody looked* — and this holds them apart by the only thing a test
    /// can see: the words. A `NotPublished` reason that talks about building is
    /// an `Unbuilt` row wearing the wrong value.
    #[test]
    fn a_not_published_row_states_the_vendors_position_and_not_our_backlog() {
        // Phrases that describe work nobody has done. Each one belongs to
        // `Unbuilt` or `Unchecked`.
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

    /// **A channel the vendor publishes but Devplane does not read renders
    /// nothing, and puts its vendor on no optimistic list.**
    ///
    /// Answering otherwise would place a vendor among the *unproved* — which
    /// means *we read this and nobody has run it* — and that is a promise about
    /// a code path which does not exist.
    #[test]
    fn a_published_channel_nobody_built_is_not_a_channel_anything_renders() {
        assert!(!Reach::Unbuilt.may_render());
        assert!(!Reach::Unchecked.may_render());
        assert!(Reach::Unproved.may_render(), "unproved still renders");

        let unbuilt: Vec<&str> = watched()
            .iter()
            .filter(|r| r.reach == Reach::Unbuilt)
            .map(|r| r.vendor)
            .collect();
        assert!(
            !unbuilt.is_empty(),
            "no row is `unbuilt`, so this check is inert — the state exists because OpenCode is in it"
        );
        let w = watching();
        for v in unbuilt {
            assert!(
                !w.watched.contains(&v) && !w.unproved.contains(&v),
                "`{v}` has a published channel nobody built and is being reported as watched"
            );
        }
    }

    /// **The four reaches read as four different things.**
    ///
    /// The words are what carries this — `devplane doctor` paints two of them
    /// the same, deliberately, because colour may never be the only thing
    /// carrying a distinction.
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

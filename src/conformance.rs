//! What this gate is, and how much of it is measured.
//!
//! `devplane doctor` answers *is it working*. This answers a different
//! question, and one nothing else in the product answered: **how much should I
//! trust the thing that decides?**
//!
//! Two halves. The **age**, because the rules are written in Claude Code's own
//! syntax so the running product can be asked the same question — a claim that
//! is true on the day it runs and decays after. And the **card**, scored
//! against a published conformance profile rather than a list this project
//! wrote for itself, because a scorecard you write for yourself is a claim.
//!
//! **The failures are the point.** A card with no ❌ in it is a marketing
//! document, and the two things absent here — externalised evidence, and
//! anything a tool call starts rather than is — are the honest boundary of what
//! a tool-call gate can promise.

use crate::core::policy::{ROWS_CLEARED_THROUGH, VERIFIED_AGAINST};

/// How far a property has got.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Met {
    Yes,
    Partly,
    No,
}

impl Met {
    pub fn mark(self) -> &'static str {
        match self {
            Met::Yes => "✓",
            Met::Partly => "~",
            Met::No => "✗",
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Met::Yes => "met",
            Met::Partly => "partial",
            Met::No => "absent",
        }
    }
}

pub struct Property {
    pub name: &'static str,
    pub met: Met,
    /// One sentence, and for a `Partly` or a `No` it says what is missing
    /// rather than what is present.
    pub note: &'static str,
}

/// The six properties of the EBL-Core execution-boundary profile
/// ([arXiv:2609.11596](https://arxiv.org/abs/2609.11596)), plus the two from the
/// older kernel argument that it does not name and that were load-bearing here.
///
/// Scored against somebody else's list on purpose. An earlier version of this
/// card was four properties chosen here, which is a claim wearing a
/// scorecard's clothes.
pub const PROPERTIES: &[Property] = &[
    Property {
        name: "action binding",
        met: Met::Yes,
        note: "every decision row names the rule that answered it, and a driven agent's request is \
               classified from the protocol rather than from its prose",
    },
    Property {
        name: "policy non-weakening",
        met: Met::Partly,
        note: "a project cannot widen a machine-wide prohibition and an agent cannot edit the rules \
               it works under — but a `git push` inside a script the agent wrote a turn ago is not a \
               tool call, and no hook sees it. The gate is a policy layer; the vendor's sandbox is \
               the boundary",
    },
    Property {
        name: "deterministic adjudication",
        met: Met::Yes,
        note: "a total, synchronous, in-process function with no model in it and no network in it; \
               the build fails if anything under core/ grows a way to wait",
    },
    Property {
        name: "derivation verification",
        met: Met::Yes,
        note: "the rule a verdict names, re-evaluated alone, must produce that verdict — a right \
               answer with a fabricated authority is the one failure this log cannot survive",
    },
    Property {
        name: "evidence handling",
        met: Met::Partly,
        note: "a gate's exit code, its output and the commit are recorded, and a work's \
               specification is stamped as it was; what is missing is a certificate a reviewer can \
               recompute without trusting this tool",
    },
    Property {
        name: "grant lifecycle",
        met: Met::Partly,
        note: "a rule is a standing grant with no expiry and no revocation beyond editing the file. \
               There is no issuer, which is the right answer for a laptop and the wrong one for the \
               profile's intended setting",
    },
    Property {
        name: "process separation",
        met: Met::Yes,
        note: "the gate is a command hook: its own process, its own files, no daemon in the path. \
               Any control inside the agent's address space is reachable by inputs that influence it",
    },
    Property {
        name: "externalised evidence",
        met: Met::No,
        note: "the decision log is append-only and never pruned, and it is a table anyone with the \
               file can edit. Signing and anchoring stay out: one machine, no egress",
    },
];

/// The tools a rule in this table can speak for.
pub const GOVERNS: &[&str] = &[
    "Bash",
    "PowerShell",
    "Monitor",
    "Read/Grep/Glob/LSP",
    "Edit/Write/NotebookEdit",
    "WebFetch",
    "Agent",
    "mcp__*",
];

/// How many releases the full measurement is behind, when that is countable.
pub fn behind(running: Option<&str>) -> Option<u64> {
    crate::core::policy::releases_ahead(running?, VERIFIED_AGAINST)
}

pub fn as_json(running: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "measured_against": VERIFIED_AGAINST,
        "rows_cleared_through": ROWS_CLEARED_THROUGH,
        "running_here": running,
        "releases_behind": behind(running),
        "governs": GOVERNS,
        "profile": "EBL-Core (arXiv:2609.11596)",
        "properties": PROPERTIES.iter().map(|p| serde_json::json!({
            "name": p.name, "met": p.met.as_str(), "note": p.note,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The card shows what is missing.
    ///
    /// The property that keeps this honest as the code improves: a conformance
    /// report with no failures in it is a marketing document, and the moment
    /// this passes trivially somebody has started scoring themselves.
    #[test]
    fn the_card_names_at_least_one_thing_this_gate_does_not_do() {
        assert!(
            PROPERTIES.iter().any(|p| p.met != Met::Yes),
            "a card with no ✗ and no ~ in it is not a measurement"
        );
    }

    /// Anything short of met says what is missing.
    #[test]
    fn an_unmet_property_says_what_is_absent_rather_than_what_is_present() {
        for p in PROPERTIES.iter().filter(|p| p.met != Met::Yes) {
            let n = p.note;
            assert!(
                n.contains("missing")
                    || n.contains("no ")
                    || n.contains("not ")
                    || n.contains("stay out")
                    || n.contains("cannot"),
                "`{}` is not met and its note does not say what is absent: {n}",
                p.name
            );
        }
    }

    #[test]
    fn the_two_floors_are_reported_and_the_gap_is_counted_only_when_countable() {
        // Three patches past whatever the baseline is, derived rather than
        // written down: a green run moving the baseline is the one event this
        // constant exists for, and it must not leave a test asserting the old
        // gap.
        let mut parts: Vec<u64> = VERIFIED_AGAINST
            .split('.')
            .map(|p| p.parse().expect("the baseline is three numbers"))
            .collect();
        *parts.last_mut().expect("a patch number") += 3;
        let three_ahead = parts
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(".");

        let v = as_json(Some(&three_ahead));
        assert_eq!(v["measured_against"], VERIFIED_AGAINST);
        assert_eq!(v["releases_behind"], 3);
        // Across a minor boundary nothing here can count, and a number would be
        // a guess in the one field that exists to be trusted.
        assert!(as_json(Some("2.2.0"))["releases_behind"].is_null());
        // Nothing reporting is not the same as no gap.
        assert!(as_json(None)["releases_behind"].is_null());
        assert!(as_json(None)["running_here"].is_null());
    }
}

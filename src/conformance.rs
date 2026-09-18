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

use crate::core::policy::{
    Cadence, ROWS_CLEARED_THROUGH, ROWS_MEASURED_THROUGH, VERIFIED_AGAINST, VERIFIED_ON, cadence,
};

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

/// One compatibility floor: a release, with what it claims and what it does not.
///
/// There are three and they are never combined. Each costs a different amount
/// to advance and each says something different, so a single "compatibility"
/// figure derived from them would hide the only thing worth knowing — *which*
/// of the three claims is the one being relied on.
pub struct Floor {
    /// Stable key for the JSON payload.
    pub key: &'static str,
    pub label: &'static str,
    pub release: &'static str,
    pub claims: &'static str,
    /// The half a reader actually needs, and the half nobody else prints.
    pub excludes: &'static str,
}

/// The three floors, in cost order: cheapest claim first.
///
/// Read in this order on purpose. A reader scanning down meets the weakest
/// claim first and the strongest last, so the expensive one is never mistaken
/// for the cheap one that happens to be a bigger number.
pub const FLOORS: &[Floor] = &[
    Floor {
        key: "rows_cleared_through",
        label: "read",
        release: ROWS_CLEARED_THROUGH,
        claims: "every rule row the vendor announced up to here has a written disposition",
        excludes: "does not claim any of it was checked against the running product",
    },
    Floor {
        key: "rows_measured_through",
        label: "measured",
        release: ROWS_MEASURED_THROUGH,
        claims: "every rule row up to here produced a probe the running vendor agreed with",
        excludes: "measured only what the vendor announced; says nothing about the rest of the matcher",
    },
    Floor {
        key: "verified_against",
        label: "compatibility",
        release: VERIFIED_AGAINST,
        claims: "the full differential matrix ran green on both axes",
        excludes: "does not claim the shapes it runs are complete, nor that skipped shapes are clean",
    },
];

/// What a per-release measurement does **not** establish, printed beside its
/// green verdict rather than left for a reader to work out.
///
/// A run built from release notes inherits every blind spot those notes have,
/// and this is not hypothetical: the most recent widening this gate was found
/// to have came from a shape no changelog row asked for. A green verdict that
/// does not say this is spending trust it has not earned — the same defect as
/// an explanation naming a rule that did not fire.
pub const MEASURES_ONLY_WHAT_WAS_ANNOUNCED: &str = "a per-release measurement covers only what the vendor announced; it does not \
     establish that the rest of the matcher still agrees";

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

/// Today, as `YYYY-MM-DD`, for the day half of the cadence.
///
/// Lives here rather than in `core` because reading a clock is I/O and
/// `tests/purity.rs` will not have it. The floors and the arithmetic stay pure;
/// only the question *what day is it* crosses out of the process.
pub fn today() -> String {
    jiff::Timestamp::now().to_string()[..10].to_string()
}

/// The full matrix's two bounds. `today` is passed in because `core` may not
/// read a clock.
pub fn matrix_cadence(running: Option<&str>, today: Option<&str>) -> Cadence {
    cadence(running, today)
}

pub fn as_json(running: Option<&str>, today: Option<&str>) -> serde_json::Value {
    let c = matrix_cadence(running, today);
    serde_json::json!({
        "verified_against": VERIFIED_AGAINST,
        "rows_cleared_through": ROWS_CLEARED_THROUGH,
        "rows_measured_through": ROWS_MEASURED_THROUGH,
        // The old name for the compatibility floor, kept so a script written
        // against it does not break. It is retired in the prose because it
        // became actively confusable the day a *measured* floor arrived:
        // `measured_against` is the expensive claim and `rows_measured_through`
        // is the cheap one, which is precisely the collapse this feature exists
        // to prevent.
        "measured_against": VERIFIED_AGAINST,
        "verified_on": VERIFIED_ON,
        // Three claims, each with its own scope. Never a fourth field derived
        // from them: the absence of a combined figure is a property with a test.
        "floors": FLOORS.iter().map(|f| serde_json::json!({
            "key": f.key, "label": f.label, "release": f.release,
            "claims": f.claims, "excludes": f.excludes,
        })).collect::<Vec<_>>(),
        // Two bounds, reported apart. A "days until due" would hide which one
        // is the reason the matrix is owed.
        "full_matrix_cadence": {
            "releases_elapsed": c.releases,
            "releases_limit": c.releases_limit,
            "days_elapsed": c.days,
            "days_limit": c.days_limit,
            "overdue": c.overdue(),
        },
        "measures_only_what_was_announced": MEASURES_ONLY_WHAT_WAS_ANNOUNCED,
        "running_here": running,
        "releases_behind": behind(running),
        // The full picture, because `releases_behind` is `null` for three
        // different situations and only one of them is "nothing to say".
        "version_gap": running.map(|v| {
            let g = crate::core::policy::gap(v, VERIFIED_AGAINST);
            serde_json::json!({
                "state": match g {
                    crate::core::policy::Gap::At => "at",
                    crate::core::policy::Gap::Ahead(_) => "ahead",
                    crate::core::policy::Gap::Behind(_) => "behind",
                    crate::core::policy::Gap::Uncountable => "uncountable",
                },
                "releases": match g {
                    crate::core::policy::Gap::Ahead(n) | crate::core::policy::Gap::Behind(n) => Some(n),
                    _ => None,
                },
                "says": g.says(),
            })
        }),
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

        let v = as_json(Some(&three_ahead), Some(VERIFIED_ON));
        assert_eq!(v["verified_against"], VERIFIED_AGAINST);
        assert_eq!(
            v["measured_against"], VERIFIED_AGAINST,
            "the old name still answers"
        );
        assert_eq!(v["releases_behind"], 3);
        // Across a minor boundary nothing here can count, and a number would be
        // a guess in the one field that exists to be trusted.
        assert!(as_json(Some("2.2.0"), None)["releases_behind"].is_null());
        // Nothing reporting is not the same as no gap.
        assert!(as_json(None, None)["releases_behind"].is_null());
        assert!(as_json(None, None)["running_here"].is_null());
    }
}

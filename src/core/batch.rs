//! A fan-out: one person's intent, sent once, to many targets.
//!
//! **Six runs from one prompt are not six things to review.** They are one
//! thing that happened six times, and the review surface has to say so — one
//! row, six outcomes, with the failures and the questions above the successes.
//!
//! # The three things this module refuses
//!
//! **There is no batch-level verdict.** No percentage, no `n/m`, no pass rate,
//! no health colour. A batch reports each member's own outcome and nothing
//! else, because a summary over six repositories is a number that hides which
//! one needs you — and *four green, one red, one asking* is the normal case
//! rather than a failure to be averaged away.
//!
//! **No position merges, and there is no setting that adds one.** The dial
//! stops at opening a pull request. A control plane that merged would be the
//! only party in the loop with no objection to it.
//!
//! **No position weakens a permission.** A call inside a batch is decided by
//! exactly the rules that decide it outside one, and the same rule is credited.
//! This is the property every comparable tool trades away to get unattended
//! operation, and it is the one thing this feature must not.
//!
//! Everything here is pure; the half that starts processes is `crate::batch`.

use crate::core::ids::{BatchId, ProjectId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// The dial
// ---------------------------------------------------------------------------

/// How far a fan-out may go without you.
///
/// **Per dispatch, never a global setting.** A dial somebody set once and
/// forgot is a dial that decides things nobody is thinking about; this is
/// chosen for each fan-out, with the number of targets in front of the person
/// choosing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum Position {
    /// Nothing runs. Each target opens with the prompt typed and **not sent**,
    /// through the vendor's own deep link.
    ///
    /// Six terminals holding a readable prompt is a better first version than
    /// six running agents, and it is the version that needs no new safety
    /// argument: the person sends, the vendor warns, and the vendor's own
    /// permission rules and trust prompts apply exactly as always.
    Draft,
    /// The agent works; the project's gates run when it stops. A failing gate,
    /// a prohibited permission, a question and the pull request all still stop.
    ToGate,
    /// The above, plus opening a pull request.
    ToPullRequest,
}

impl Position {
    /// The spelling that crosses the wire. Asked of serde in a test rather than
    /// reconstructed from `Debug`, because agreeing with the protocol for every
    /// variant anybody has looked at is a coincidence and not a rule.
    pub fn as_str(self) -> &'static str {
        match self {
            Position::Draft => "draft",
            Position::ToGate => "to_gate",
            Position::ToPullRequest => "to_pull_request",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "draft" => Position::Draft,
            "to_gate" | "gate" => Position::ToGate,
            "to_pull_request" | "pr" => Position::ToPullRequest,
            _ => return None,
        })
    }

    /// Whether anything starts. The one question the safety argument turns on.
    pub fn starts_anything(self) -> bool {
        !matches!(self, Position::Draft)
    }

    /// What it will and will not do, for the composer.
    pub fn says(self) -> &'static str {
        match self {
            Position::Draft => "nothing runs — each project opens with the prompt typed, not sent",
            Position::ToGate => {
                "the agent works and your gates run; a question or a failing gate stops it"
            }
            Position::ToPullRequest => {
                "the above, and it opens a pull request. Merging is never included"
            }
        }
    }
}

/// Above this many targets, draft is chosen **for** the person.
///
/// Three is where a mistake stops being a wrong row and starts being several
/// repositories, and the number is named here rather than inlined so the
/// boundary is one edit and one test.
pub const DRAFT_ABOVE: usize = 3;

/// The position a fan-out starts at, and whether the count chose it.
///
/// `forced` exists so a surface can say *draft was selected for you* rather
/// than letting somebody discover that their choice was overridden. A default
/// nobody is told about is a decision taken on their behalf, which is the
/// thing this product exists to make visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Chosen {
    pub position: Position,
    pub forced: bool,
}

pub fn default_position(target_count: usize) -> Chosen {
    // Draft either way; above the bound it is also the only thing on offer.
    // This was two arms returning the same position and differing in one bool,
    // which read as though the count chose between two modes when it chooses
    // between *asking* and *not asking*.
    Chosen {
        position: Position::Draft,
        forced: target_count > DRAFT_ABOVE,
    }
}

// ---------------------------------------------------------------------------
// Preflight
// ---------------------------------------------------------------------------

/// Why one target cannot take *this* dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum PreflightReason {
    /// Nobody has looked at what this repository would allow an agent to do.
    Untrusted,
    /// Uncommitted changes: starting here mixes the agent's work with somebody's.
    DirtyWorktree,
    /// The chosen agent cannot run in this target.
    NoSuchAgent,
    /// Its `devplane.toml` does not parse, so its prohibitions are **not in
    /// force** — which is the dangerous direction, and the reason this is a
    /// refusal rather than a warning.
    ConfigWillNotLoad,
    /// Over the configured ceiling, at a running position only.
    OverCeiling,
}

impl PreflightReason {
    pub fn as_str(self) -> &'static str {
        match self {
            PreflightReason::Untrusted => "untrusted",
            PreflightReason::DirtyWorktree => "dirty_worktree",
            PreflightReason::NoSuchAgent => "no_such_agent",
            PreflightReason::ConfigWillNotLoad => "config_will_not_load",
            PreflightReason::OverCeiling => "over_ceiling",
        }
    }

    /// Every reason this system can produce. Used by a test asserting each is
    /// reachable: **a refusal nothing can produce is a refusal nobody will
    /// see**, and it would sit in the code looking like coverage.
    pub const ALL: &'static [PreflightReason] = &[
        PreflightReason::Untrusted,
        PreflightReason::DirtyWorktree,
        PreflightReason::NoSuchAgent,
        PreflightReason::ConfigWillNotLoad,
        PreflightReason::OverCeiling,
    ];

    /// What a person reads, and what to do about it.
    pub fn says(self) -> &'static str {
        match self {
            PreflightReason::Untrusted => "not trusted",
            PreflightReason::DirtyWorktree => "uncommitted changes",
            PreflightReason::NoSuchAgent => "cannot run that agent",
            PreflightReason::ConfigWillNotLoad => "its devplane.toml will not parse",
            PreflightReason::OverCeiling => "over the parallel-run ceiling",
        }
    }

    pub fn fix(self) -> &'static str {
        match self {
            PreflightReason::Untrusted => "devplane trust <path>",
            PreflightReason::DirtyWorktree => "commit or stash first",
            PreflightReason::NoSuchAgent => "pick another agent, or install it",
            PreflightReason::ConfigWillNotLoad => "devplane check <path>",
            PreflightReason::OverCeiling => "send fewer, or raise the ceiling",
        }
    }
}

/// Everything known about one target, with no disk in sight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFacts {
    pub trusted: bool,
    pub worktree_clean: bool,
    pub agent_available: bool,
    pub config_loads: bool,
    /// Runs already in flight across the machine, plus this one's place in the
    /// batch — so the ceiling is a fact about the fan-out rather than about one
    /// project.
    pub would_exceed_ceiling: bool,
}

/// Whether one target can take this dispatch, and why not.
///
/// **Ordered so the most actionable refusal wins.** A target that is both
/// untrusted and dirty is reported as untrusted: trusting it is the thing that
/// has to happen first, and naming the second reason would send somebody to
/// commit changes in a repository they have not yet agreed to run agents in.
///
/// `Draft` starts nothing, so the ceiling never applies to it — that exemption
/// is the caller's, expressed by never setting `would_exceed_ceiling`.
pub fn preflight(f: &TargetFacts) -> Option<PreflightReason> {
    if !f.trusted {
        return Some(PreflightReason::Untrusted);
    }
    if !f.config_loads {
        return Some(PreflightReason::ConfigWillNotLoad);
    }
    if !f.agent_available {
        return Some(PreflightReason::NoSuchAgent);
    }
    if !f.worktree_clean {
        return Some(PreflightReason::DirtyWorktree);
    }
    if f.would_exceed_ceiling {
        return Some(PreflightReason::OverCeiling);
    }
    None
}

/// One target and what the preflight said about it, with the time it was said.
///
/// **A snapshot, not a promise.** A target can become undispatchable between
/// the preflight and the send — somebody commits, somebody edits a config — so
/// the time is carried and a surface says *as of* rather than implying a
/// guarantee it cannot make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "PreflightFinding", export, export_to = "wire/")
)]
pub struct Finding {
    pub project: ProjectId,
    pub refusal: Option<PreflightReason>,
    /// Fields this target's vendor would reject, from the library's portability
    /// report. **A warning and never a refusal**: the artefact still works in
    /// the tool that wrote it, and the documented error is about leaving it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub would_lose_fields: Vec<String>,
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    pub at: Timestamp,
}

impl Finding {
    pub fn accepted(&self) -> bool {
        self.refusal.is_none()
    }
}

// ---------------------------------------------------------------------------
// The batch
// ---------------------------------------------------------------------------

/// What kind of thing this batch is.
///
/// **Two kinds, not a nullable member list**, and the distinction is
/// load-bearing. Modelling a draft as *a dispatched batch whose members have
/// not arrived* would make a draft nobody sent indistinguishable from six runs
/// that failed to start — which is exactly the collapse this feature exists to
/// prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
// Named for its subject on the wire, where there are no modules to tell one
// `Kind` from another.
#[cfg_attr(
    feature = "typescript",
    ts(rename = "BatchKind", export, export_to = "wire/")
)]
pub enum Kind {
    /// Composition recorded, links produced, nothing started. **Complete on
    /// creation**: it never gains members and never waits for them.
    Drafted,
    /// One member per accepted target.
    Dispatched,
}

/// One person's intent, sent once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Batch {
    pub id: BatchId,
    /// Stored once, not per member.
    pub prompt: String,
    /// The library artefact it started from, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub position: Position,
    pub kind: Kind,
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    pub sent_at: Timestamp,
    /// The dispatch authority: who sent it. One authority for the batch, and
    /// one per run — *you approved the plan* and *a rule allowed what it then
    /// did* are different facts about the same afternoon.
    pub sent_by: String,
    /// **Every project chosen, including the ones the preflight refused.** A
    /// record that dropped them would answer *what did I send this to* with the
    /// subset that happened to work.
    pub targets: Vec<Finding>,
}

impl Batch {
    /// Targets the preflight accepted.
    pub fn accepted(&self) -> impl Iterator<Item = &Finding> {
        self.targets.iter().filter(|t| t.accepted())
    }

    /// How many runs this costs, **in runs**.
    ///
    /// Never in currency: a dollar estimate for a model whose price changed
    /// last week is asserted rather than measured. *Six projects × one run* is
    /// a number a person can reason about.
    pub fn cost_in_runs(&self) -> usize {
        self.accepted().count()
    }
}

/// What a batch currently is.
///
/// A pure function over its members, so the four empties are a test table
/// rather than four fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum State {
    /// `kind = Drafted`. Complete on creation.
    Drafted,
    /// Every target was refused at preflight. **Nothing ever ran.**
    NeverStarted,
    /// At least one member is unfinished.
    Running,
    /// Dispatched, and its members are neither finished nor running — the
    /// daemon stopped underneath them.
    Interrupted,
    /// Every member is terminal. **Rendered as its members, never as a score.**
    Finished,
}

impl State {
    /// One sentence each, and **no two are the same**.
    ///
    /// The four empty-looking states are the thing this feature is most likely
    /// to collapse: *nobody sent it*, *everything was refused*, *it was
    /// interrupted* and *it finished and did nothing* all render as an empty
    /// member list, and they are four different facts.
    pub fn says(self) -> &'static str {
        match self {
            State::Drafted => "composed and not sent — each project was opened with the prompt",
            State::NeverStarted => "every target was refused; nothing ran",
            State::Running => "still going",
            State::Interrupted => "the daemon stopped while this was running",
            State::Finished => "every target is done",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            State::Drafted => "drafted",
            State::NeverStarted => "never_started",
            State::Running => "running",
            State::Interrupted => "interrupted",
            State::Finished => "finished",
        }
    }

    /// Beside `as_str`, so the two spellings cannot drift apart. A test asserts
    /// every variant round-trips.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "drafted" => State::Drafted,
            "never_started" => State::NeverStarted,
            "running" => State::Running,
            "interrupted" => State::Interrupted,
            "finished" => State::Finished,
            _ => return None,
        })
    }
}

/// One member's terminal-ness, as the caller sees it.
///
/// Deliberately not a copy of `Phase`: this module needs to know *finished or
/// not*, and importing a phase enum here would tie the batch's own states to
/// every future change in a work item's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    pub work: WorkIdRef,
    pub terminal: bool,
    /// Whether anything is actually in flight for it right now. A member that
    /// is neither terminal nor live is what an interruption looks like.
    pub live: bool,
}

/// A cheap stand-in so this module needs no lifetime on `WorkId`.
pub type WorkIdRef = u64;

/// What a batch is, from its members.
pub fn state(kind: Kind, accepted_targets: usize, members: &[Member]) -> State {
    if kind == Kind::Drafted {
        return State::Drafted;
    }
    if accepted_targets == 0 {
        return State::NeverStarted;
    }
    if members.iter().any(|m| !m.terminal && m.live) {
        return State::Running;
    }
    if members.iter().any(|m| !m.terminal && !m.live) {
        return State::Interrupted;
    }
    State::Finished
}

/// Members ordered the way a person reads them: what needs you, then what
/// failed, then the rest.
///
/// **The batch itself is uncoloured.** Partial failure is the normal case — four
/// green, one red, one asking a question is what a fan-out looks like — so
/// colouring the batch would be inventing the verdict this feature refuses.
pub fn order<T: Copy>(members: &[(T, bool, bool)]) -> Vec<(T, bool, bool)> {
    // `(member, needs_you, failed)`.
    let mut v = members.to_vec();
    v.sort_by_key(|(_, needs_you, failed)| (!*needs_you, !*failed));
    v
}

/// The members of one batch, for the surfaces that list them.
///
/// Generic over what a member *is* so the one rule — *this work carries this
/// batch id* — has one home. It had two: this function, which nothing called,
/// and the same filter inlined in the API's renderer, which is the copy that
/// decided what a person saw.
pub fn members_of<'a, T>(
    batch: &BatchId,
    works: impl Iterator<Item = (T, Option<&'a BatchId>)>,
) -> Vec<T> {
    works
        .filter(|(_, b)| *b == Some(batch))
        .map(|(w, _)| w)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> TargetFacts {
        TargetFacts {
            trusted: true,
            worktree_clean: true,
            agent_available: true,
            config_loads: true,
            would_exceed_ceiling: false,
        }
    }

    /// A wire enum is asked of serde rather than reconstructed from `Debug`.
    #[test]
    fn the_wire_spellings_are_what_serde_writes() {
        for p in [Position::Draft, Position::ToGate, Position::ToPullRequest] {
            let json = serde_json::to_string(&p).unwrap();
            assert_eq!(json, format!("\"{}\"", p.as_str()), "{p:?}");
            assert_eq!(Position::parse(p.as_str()), Some(p));
        }
        for r in PreflightReason::ALL {
            let json = serde_json::to_string(r).unwrap();
            assert_eq!(json, format!("\"{}\"", r.as_str()), "{r:?}");
        }
        for s in [
            State::Drafted,
            State::NeverStarted,
            State::Running,
            State::Interrupted,
            State::Finished,
        ] {
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.as_str())
            );
            assert_eq!(State::parse(s.as_str()), Some(s), "{s:?}");
        }
    }

    #[test]
    fn draft_is_chosen_for_you_above_three_targets_and_not_at_three() {
        assert!(!default_position(1).forced);
        assert!(!default_position(3).forced, "three is not above three");
        assert!(default_position(4).forced, "four is");
        assert!(default_position(60).forced);
        // And the position itself is draft either way: the flag is about
        // whether the *count* decided, not about what was decided.
        for n in [1, 3, 4, 60] {
            assert_eq!(default_position(n).position, Position::Draft);
        }
    }

    #[test]
    fn only_draft_starts_nothing() {
        assert!(!Position::Draft.starts_anything());
        assert!(Position::ToGate.starts_anything());
        assert!(Position::ToPullRequest.starts_anything());
    }

    /// **No position merges, and the enum cannot express one.**
    #[test]
    fn there_is_no_position_that_merges() {
        for p in [Position::Draft, Position::ToGate, Position::ToPullRequest] {
            let said = p.says().to_lowercase();
            assert!(
                !said.contains("merge") || said.contains("never"),
                "{p:?} says something about merging that is not a refusal: {said}"
            );
        }
        // The furthest position names the limit rather than leaving it implied.
        assert!(
            Position::ToPullRequest.says().contains("Merging is never"),
            "the last position must say where it stops"
        );
    }

    #[test]
    fn a_clean_trusted_target_is_accepted_and_each_refusal_is_reachable() {
        assert_eq!(preflight(&facts()), None);

        let cases = [
            (
                TargetFacts {
                    trusted: false,
                    ..facts()
                },
                PreflightReason::Untrusted,
            ),
            (
                TargetFacts {
                    config_loads: false,
                    ..facts()
                },
                PreflightReason::ConfigWillNotLoad,
            ),
            (
                TargetFacts {
                    agent_available: false,
                    ..facts()
                },
                PreflightReason::NoSuchAgent,
            ),
            (
                TargetFacts {
                    worktree_clean: false,
                    ..facts()
                },
                PreflightReason::DirtyWorktree,
            ),
            (
                TargetFacts {
                    would_exceed_ceiling: true,
                    ..facts()
                },
                PreflightReason::OverCeiling,
            ),
        ];
        for (f, want) in &cases {
            assert_eq!(preflight(f), Some(*want), "{f:?}");
        }

        // **Every reason the system knows is reachable.** One that nothing can
        // produce sits in the code looking like coverage.
        let reachable: Vec<PreflightReason> = cases.iter().map(|(_, r)| *r).collect();
        for r in PreflightReason::ALL {
            assert!(
                reachable.contains(r),
                "{r:?} cannot be produced by anything"
            );
        }
    }

    /// The order exists so the fix a person is sent to do is the one that has
    /// to happen first.
    #[test]
    fn an_untrusted_and_dirty_target_is_reported_as_untrusted() {
        let both = TargetFacts {
            trusted: false,
            worktree_clean: false,
            ..facts()
        };
        assert_eq!(
            preflight(&both),
            Some(PreflightReason::Untrusted),
            "committing changes in a repository you have not agreed to run agents in \
             is the wrong first instruction"
        );
    }

    /// **The four empties, and no two read the same.**
    #[test]
    fn the_states_that_all_look_empty_are_four_different_sentences() {
        let done = Member {
            work: 1,
            terminal: true,
            live: false,
        };
        let going = Member {
            work: 2,
            terminal: false,
            live: true,
        };
        let stranded = Member {
            work: 3,
            terminal: false,
            live: false,
        };

        assert_eq!(state(Kind::Drafted, 0, &[]), State::Drafted);
        assert_eq!(state(Kind::Dispatched, 0, &[]), State::NeverStarted);
        assert_eq!(state(Kind::Dispatched, 2, &[done, going]), State::Running);
        assert_eq!(
            state(Kind::Dispatched, 2, &[done, stranded]),
            State::Interrupted
        );
        assert_eq!(state(Kind::Dispatched, 1, &[done]), State::Finished);
        // Finished having produced nothing is still finished, and is not the
        // same sentence as never having started.
        assert_eq!(state(Kind::Dispatched, 1, &[]), State::Finished);

        let all = [
            State::Drafted,
            State::NeverStarted,
            State::Running,
            State::Interrupted,
            State::Finished,
        ];
        let mut said: Vec<_> = all.iter().map(|s| s.says()).collect();
        said.sort_unstable();
        said.dedup();
        assert_eq!(said.len(), all.len(), "two states read the same");
    }

    /// A drafted batch is complete on creation and never becomes running.
    #[test]
    fn a_draft_never_becomes_a_dispatched_batch_with_missing_members() {
        let going = Member {
            work: 1,
            terminal: false,
            live: true,
        };
        assert_eq!(
            state(Kind::Drafted, 6, &[going]),
            State::Drafted,
            "a draft's kind decides it; members cannot drag it into `Running`"
        );
    }

    #[test]
    fn what_needs_you_comes_first_then_what_failed() {
        // `(id, needs_you, failed)`
        let members = [
            ('a', false, false),
            ('b', false, true),
            ('c', true, false),
            ('d', false, false),
        ];
        let got: Vec<char> = order(&members).into_iter().map(|(id, ..)| id).collect();
        assert_eq!(got, ['c', 'b', 'a', 'd']);
        // Stable among equals: two rows that are neither waiting nor failed
        // keep the order they were given, so a list does not reshuffle itself
        // between two refreshes that learned nothing.
        assert_eq!(order(&members).len(), members.len());
    }

    #[test]
    fn cost_is_counted_in_runs_and_refused_targets_are_kept() {
        let at = Timestamp::now();
        let b = Batch {
            id: BatchId::new("b1"),
            prompt: "bump deps".into(),
            template: None,
            position: Position::ToGate,
            kind: Kind::Dispatched,
            sent_at: at,
            sent_by: "hupe".into(),
            targets: vec![
                Finding {
                    project: ProjectId::new("a"),
                    refusal: None,
                    would_lose_fields: vec![],
                    at,
                },
                Finding {
                    project: ProjectId::new("b"),
                    refusal: Some(PreflightReason::Untrusted),
                    would_lose_fields: vec![],
                    at,
                },
                Finding {
                    project: ProjectId::new("c"),
                    refusal: None,
                    would_lose_fields: vec!["argument-hint".into()],
                    at,
                },
            ],
        };
        assert_eq!(b.cost_in_runs(), 2, "two accepted, counted in runs");
        assert_eq!(
            b.targets.len(),
            3,
            "the refused target is kept on the record"
        );
        // A portability warning is not a refusal.
        assert!(b.targets[2].accepted());
    }

    /// **No aggregate anywhere.** Asserted over the vocabulary this module can
    /// produce, because the summary is what a later pass helpfully adds.
    #[test]
    fn nothing_this_module_can_say_is_an_aggregate() {
        let mut vocabulary: Vec<String> = Vec::new();
        for s in [
            State::Drafted,
            State::NeverStarted,
            State::Running,
            State::Interrupted,
            State::Finished,
        ] {
            vocabulary.push(s.says().into());
            vocabulary.push(s.as_str().into());
        }
        for p in [Position::Draft, Position::ToGate, Position::ToPullRequest] {
            vocabulary.push(p.says().into());
        }
        for r in PreflightReason::ALL {
            vocabulary.push(r.says().into());
            vocabulary.push(r.fix().into());
        }
        for word in &vocabulary {
            let lower = word.to_lowercase();
            for banned in ["%", "pass rate", "success rate", "healthy", "score"] {
                assert!(!lower.contains(banned), "`{banned}` in {word:?}");
            }
            let b: Vec<char> = lower.chars().collect();
            assert!(
                !b.windows(3)
                    .any(|w| w[0].is_ascii_digit() && w[1] == '/' && w[2].is_ascii_digit()),
                "an n/m ratio is an aggregate: {word:?}"
            );
        }
    }
}

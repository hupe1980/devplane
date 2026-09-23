//! Fan-out: one prompt, six repositories, one reviewable row.
//!
//! Three of these tests are **absence checks**, and an absence check is the
//! only way an absence stays true. *Nothing merges*, *no position weakens a
//! permission* and *no surface prints an aggregate* are all properties that a
//! reasonable person removes by accident while adding something helpful, and
//! none of them fails loudly when it goes.

use devplane::batch;
use devplane::core::ProjectId;
use devplane::core::batch::{
    self as core_batch, Kind, Position, PreflightReason, State, TargetFacts,
};
use std::path::PathBuf;

fn target(name: &str, root: &str, trusted: bool) -> batch::Target {
    batch::Target {
        project: ProjectId::new(root),
        name: name.into(),
        root: PathBuf::from(root),
        trusted,
    }
}

fn finding(p: &str, refusal: Option<PreflightReason>) -> core_batch::Finding {
    core_batch::Finding {
        project: ProjectId::new(p),
        refusal,
        would_lose_fields: vec![],
        at: jiff::Timestamp::now(),
    }
}

/// **Six projects, one prompt, six links, and no process starts.**
///
/// Counted by processes rather than by inspecting state: the claim is about the
/// machine, and a state field saying *nothing started* is exactly what a bug
/// here would leave behind.
#[test]
fn a_draft_produces_a_link_per_target_and_starts_nothing() {
    // **This process's own children**, not the machine's process count.
    //
    // The first version snapshotted every process on the machine and compared
    // totals. That is flaky by construction — a cargo build, an editor, another
    // test binary all move the number — and it failed for a reason that had
    // nothing to do with drafting. A draft that started something would start
    // it *here*, as a child of this test.
    let me = std::process::id();
    let children = || {
        devplane::observe::procs::snapshot()
            .into_iter()
            .filter(|p| p.ppid == me)
            .count()
    };
    let before = children();

    let targets: Vec<batch::Target> = ["api", "web", "jobs", "billing", "infra", "payments"]
        .iter()
        .map(|n| target(n, &format!("/repos/{n}"), true))
        .collect();
    let findings: Vec<_> = targets
        .iter()
        .map(|t| finding(t.project.as_str(), None))
        .collect();

    let b = batch::drafted("bump deps", "tester", None, findings);
    let links = batch::draft_links(&b, &targets);

    assert_eq!(links.len(), 6, "one link per target");
    for (name, link) in &links {
        let link = link
            .as_ref()
            .unwrap_or_else(|| panic!("{name} has no link"));
        assert!(link.starts_with("claude-cli://open?cwd="), "{link}");
        assert!(link.contains("bump"), "the prompt travels: {link}");
    }
    // Six distinct targets, six distinct links.
    let mut urls: Vec<&String> = links.iter().filter_map(|(_, l)| l.as_ref()).collect();
    urls.sort();
    urls.dedup();
    assert_eq!(urls.len(), 6, "two targets produced the same link");

    let after = children();
    assert_eq!(
        after, before,
        "drafting started a child process; it must start nothing at all"
    );
}

/// **The success criterion as one test**, because three tests each covering
/// part of it can all pass while the sentence is false.
#[test]
fn one_prompt_reaches_six_projects_as_one_row_and_nothing_in_the_path_merges() {
    let names = ["api", "web", "jobs", "billing", "infra", "payments"];
    let findings: Vec<_> = names
        .iter()
        .map(|n| finding(&format!("/repos/{n}"), None))
        .collect();

    // One row.
    let b = batch::dispatched(
        "bump deps",
        "tester",
        None,
        Position::ToPullRequest,
        findings,
    );
    assert_eq!(b.targets.len(), 6, "one row, six targets");
    assert_eq!(b.cost_in_runs(), 6);
    assert_eq!(
        b.prompt, "bump deps",
        "the prompt is stored once, not per member"
    );

    // Six outcomes, and no aggregate over them.
    let json = serde_json::to_string(&b).unwrap().to_lowercase();
    for aggregate in [
        "pass_rate",
        "success_rate",
        "percent",
        "score",
        "health",
        "summary",
    ] {
        assert!(!json.contains(aggregate), "{aggregate} in {json}");
    }

    // **And nothing in the path merges.** The furthest position opens a pull
    // request and says where it stops; no variant, spelling or parse target
    // reaches a merge.
    for p in [Position::Draft, Position::ToGate, Position::ToPullRequest] {
        assert!(
            !p.as_str().contains("merge"),
            "a position named a merge: {}",
            p.as_str()
        );
    }
    for spelling in ["merge", "merged", "to_merge", "automerge", "squash"] {
        assert_eq!(
            Position::parse(spelling),
            None,
            "`{spelling}` parsed into a position"
        );
    }
}

/// The dial is per dispatch, and above three targets it is chosen **for** the
/// person — who is told so rather than discovering it.
#[test]
fn draft_is_forced_above_three_targets_and_the_person_is_told() {
    assert!(!core_batch::default_position(3).forced);
    let four = core_batch::default_position(4);
    assert!(four.forced);
    assert_eq!(four.position, Position::Draft);
}

/// **A batch whose every target was refused reads differently from one that ran
/// and did nothing.**
#[test]
fn the_four_empty_looking_states_are_four_different_facts() {
    let refused: Vec<_> = ["a", "b"]
        .iter()
        .map(|p| finding(p, Some(PreflightReason::Untrusted)))
        .collect();
    let b = batch::dispatched("p", "me", None, Position::ToGate, refused);
    assert_eq!(b.cost_in_runs(), 0);
    assert_eq!(
        core_batch::state(Kind::Dispatched, b.cost_in_runs(), &[]),
        State::NeverStarted
    );

    // Finished having produced nothing is a different sentence.
    let ran = batch::dispatched("p", "me", None, Position::ToGate, vec![finding("a", None)]);
    assert_eq!(
        core_batch::state(Kind::Dispatched, ran.cost_in_runs(), &[]),
        State::Finished
    );
    assert_ne!(State::NeverStarted.says(), State::Finished.says());
    assert_ne!(State::Drafted.says(), State::NeverStarted.says());
    assert_ne!(State::Interrupted.says(), State::Running.says());
}

/// Stopping the daemon mid-batch leaves it reading as interrupted, not as
/// finished — a member that is neither terminal nor live is what a stopped
/// daemon looks like from the outside.
#[test]
fn a_daemon_that_stopped_mid_batch_leaves_the_batch_interrupted() {
    let stranded = core_batch::Member {
        work: 1,
        terminal: false,
        live: false,
    };
    let done = core_batch::Member {
        work: 2,
        terminal: true,
        live: false,
    };
    assert_eq!(
        core_batch::state(Kind::Dispatched, 2, &[done, stranded]),
        State::Interrupted
    );
    // And it is not silently the same as finished, which is the collapse that
    // would make a half-done fan-out look complete.
    assert_ne!(
        core_batch::state(Kind::Dispatched, 2, &[done, stranded]),
        core_batch::state(Kind::Dispatched, 1, &[done])
    );
}

/// **A batch of one takes the same path as a batch of six.** Two paths means
/// the less-used one rots.
#[test]
fn a_batch_of_one_produces_the_same_record_as_a_single_dispatch() {
    let one = batch::dispatched("p", "me", None, Position::ToGate, vec![finding("a", None)]);
    let six = batch::dispatched(
        "p",
        "me",
        None,
        Position::ToGate,
        (0..6).map(|i| finding(&format!("{i}"), None)).collect(),
    );
    // Same shape, same fields, same kind — only the target count differs.
    let a: serde_json::Value = serde_json::to_value(&one).unwrap();
    let b: serde_json::Value = serde_json::to_value(&six).unwrap();
    let keys = |v: &serde_json::Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(keys(&a), keys(&b), "a batch of one is a different shape");
    assert_eq!(one.kind, six.kind);
}

/// **The most important test in the feature, and the first version of it was a
/// tautology.**
///
/// The claim is that a permission decided *inside* a fan-out is decided by
/// exactly the rules that decide it outside one, crediting the same rule. The
/// first attempt called `policy.restrictive` twice with identical arguments and
/// asserted the results matched — which they do, necessarily, and which proves
/// nothing about the batch path at all.
///
/// What actually holds the claim up is structural: **the batch path contains no
/// permission machinery whatsoever.** It cannot decide a call differently
/// because it cannot decide a call. Every dispatch goes through the existing
/// single-target path, which is the only place a verdict is produced.
///
/// So this reads the source and asserts the absence. An absence check is the
/// only way an absence stays true, and this one would fail the moment somebody
/// added a *reasonable* convenience — a `--yes`, a batch-scoped allow list, a
/// "skip prompts while fanning out".
#[test]
fn the_fan_out_path_contains_no_permission_machinery_at_all() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = ["src/batch.rs", "src/core/batch.rs", "src/cli/batch.rs"];

    // Things that would mean a fan-out can influence a verdict. `permission`
    // and `policy` are included deliberately: this path has no business
    // mentioning either, and the day it needs to is the day this claim needs
    // re-arguing rather than the day this test needs relaxing.
    let forbidden = [
        "Verdict",
        "Policy",
        "restrictive(",
        "permission",
        "allowed_tools",
        "auto_allow",
        "bypass",
        "skip-permissions",
        "dangerously",
        "--yes",
    ];

    for rel in sources {
        let text = std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        for (n, line) in text.lines().enumerate() {
            // Prose may discuss what the path refuses to do; code may not do it.
            let code = line.trim_start();
            if code.starts_with("//") || code.starts_with("///") || code.starts_with("//!") {
                continue;
            }
            for bad in forbidden {
                assert!(
                    !line.contains(bad),
                    "{rel}:{}: the fan-out path names `{bad}`, so it is no longer true that \
                     a call inside a batch meets exactly the machinery a call outside one meets\n  {line}",
                    n + 1
                );
            }
        }
    }
}

/// And the same absence over the surfaces: no flag, no field and no route
/// reaches a merge or weakens a permission.
#[test]
fn no_surface_offers_a_merge_or_a_weaker_permission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli = std::fs::read_to_string(root.join("src/cli/mod.rs")).unwrap();

    // The dispatch command's own arguments, read from its declaration.
    let start = cli.find("Dispatch {").expect("the dispatch command exists");
    let end = start + cli[start..].find("\n    },").expect("its block closes");
    let block = &cli[start..end];
    for bad in ["merge", "no_verify", "force_push", "yes", "skip"] {
        assert!(
            !block.to_lowercase().contains(bad),
            "`{bad}` is a dispatch argument:\n{block}"
        );
    }

    // And no route named for it.
    let api = std::fs::read_to_string(root.join("src/api.rs")).unwrap();
    for line in api.lines().filter(|l| l.contains(".route(")) {
        let l = line.to_lowercase();
        assert!(
            !(l.contains("batch") && l.contains("merge")),
            "a batch route reaches a merge: {line}"
        );
    }
}

/// Every refusal the system knows is reachable, and each names its own fix.
#[test]
fn every_refusal_is_reachable_and_carries_a_command() {
    let base = TargetFacts {
        trusted: true,
        worktree_clean: true,
        agent_available: true,
        config_loads: true,
        would_exceed_ceiling: false,
    };
    let produced = [
        (
            TargetFacts {
                trusted: false,
                ..base.clone()
            },
            PreflightReason::Untrusted,
        ),
        (
            TargetFacts {
                config_loads: false,
                ..base.clone()
            },
            PreflightReason::ConfigWillNotLoad,
        ),
        (
            TargetFacts {
                agent_available: false,
                ..base.clone()
            },
            PreflightReason::NoSuchAgent,
        ),
        (
            TargetFacts {
                worktree_clean: false,
                ..base.clone()
            },
            PreflightReason::DirtyWorktree,
        ),
        (
            TargetFacts {
                would_exceed_ceiling: true,
                ..base.clone()
            },
            PreflightReason::OverCeiling,
        ),
    ];
    for (facts, want) in &produced {
        assert_eq!(core_batch::preflight(facts), Some(*want));
        let line = batch::refusal_line("billing", *want, std::path::Path::new("/repos/billing"));
        assert!(line.contains("billing"), "{line}");
        assert!(line.contains('—'), "every refusal carries a fix: {line}");
    }
    for r in PreflightReason::ALL {
        assert!(
            produced.iter().any(|(_, p)| p == r),
            "{r:?} is a refusal nothing can produce, so nobody will ever see it"
        );
    }
}

/// A drafted batch records the composition and **nothing about what happened
/// next**.
/// **The portability finding is computed, and it is a warning.**
///
/// `preflight_all` set `would_lose_fields` to an empty vector unconditionally
/// for the life of the feature, so the requirement it exists for — *report, per
/// target, which fields of a chosen artefact the documented distribution paths
/// reject* — had no producer at all. The page that was supposed to render it
/// was reading a field nothing filled, which is why deleting that page lost
/// nothing and why nobody noticed.
///
/// Two halves, and the second is the one that matters: it is reported, **and it
/// never refuses**. The artefact still works in the tool that wrote it, and the
/// documented error is about leaving it.
#[test]
fn a_field_a_target_would_drop_is_a_warning_and_never_a_refusal() {
    use devplane::core::library::{Why, portability};

    // The library's own rule, so this test cannot drift from it: a key outside
    // the portable core is a documented failure.
    let report = portability(
        &[
            ("name".into(), "review-findings".into()),
            ("argument-hint".into(), "<file>".into()),
        ],
        false,
    );
    let lost: Vec<String> = report.findings.iter().map(|f| f.field.clone()).collect();
    assert_eq!(
        lost,
        vec!["argument-hint"],
        "the portable core is six fields"
    );
    assert!(
        report.findings.iter().all(|f| f.why == Why::UnexpectedKey),
        "an extra key is an unexpected key, not a malformed one"
    );

    // And the shape the composer carries it in: beside the refusal, never in
    // it. A target that would drop a field is still an accepted target.
    let f = core_batch::Finding {
        project: devplane::core::ProjectId::new("/p"),
        refusal: None,
        would_lose_fields: lost,
        at: jiff::Timestamp::now(),
    };
    assert!(
        f.accepted(),
        "a field the target would drop may not turn the target down"
    );
}

#[test]
fn nothing_infers_that_a_draft_was_sent() {
    let b = batch::drafted("p", "me", None, vec![finding("a", None)]);
    let json = serde_json::to_string(&b).unwrap().to_lowercase();
    for inferred in [
        "sent_after",
        "was_sent",
        "followed",
        "opened_at",
        "accepted_at",
        "launched",
    ] {
        assert!(!json.contains(inferred), "{inferred} in {json}");
    }
    assert_eq!(b.kind, Kind::Drafted);
}

/// Cost is stated in runs. A currency figure is asserted rather than measured.
#[test]
fn cost_is_counted_in_runs_and_never_in_money() {
    let b = batch::dispatched(
        "p",
        "me",
        None,
        Position::ToGate,
        vec![
            finding("a", None),
            finding("b", Some(PreflightReason::Untrusted)),
            finding("c", None),
        ],
    );
    assert_eq!(b.cost_in_runs(), 2);
    let json = serde_json::to_string(&b).unwrap().to_lowercase();
    for money in ["usd", "dollar", "cost_usd", "price", "$"] {
        assert!(!json.contains(money), "{money} in {json}");
    }
}

//! The done certificate: what it says, what it refuses to say, and what a
//! stranger can do with it. Mostly wording assertions, since the failure mode
//! is a true record read as a larger claim than the evidence supports.

use devplane::core::certificate::{Certificate, LIMITS, MAX_BYTES, REDERIVABLE};
use devplane::core::change::{
    Change, CommandResult, CommitStamp, Completion, GateReport, Outcome, Reach, SpecStamp, Waiting,
};

fn change() -> Change {
    Change::new(
        devplane::core::ids::ProjectId::new("p1"),
        "Add rate limiting".into(),
        "do the thing".into(),
    )
}

fn stamp(commit: Option<&str>, clean: bool, reach: Reach) -> CommitStamp {
    CommitStamp {
        commit: commit.map(str::to_string),
        tree: commit.map(|c| format!("tree-of-{c}")),
        branch: Some("change/rate-limiting".into()),
        clean,
        changed_files: if clean { 0 } else { 3 },
        reach,
        remote: Some("git@github.com:acme/widgets.git".into()),
    }
}

fn report(gate: &str, attempt: u32, commands: Vec<CommandResult>) -> GateReport {
    GateReport {
        gate: gate.into(),
        at: jiff::Timestamp::now(),
        duration_ms: 10,
        commands,
        attempt,
        spec: None,
        commit: Some(stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    }
}

fn finished(mut w: Change, basis: Completion) -> Change {
    w.completion = Some(basis);
    w
}

fn passing() -> Change {
    let mut w = change();
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("cargo test", 0)],
    ));
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    finished(w, basis)
}

// ── The claim a stranger re-derives ────────────────────────────────────────

#[test]
fn the_document_names_the_commit_the_commands_and_how_to_run_them() {
    let w = passing();
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("4f2a9c1e"), "the commit is not named:\n{md}");
    assert!(md.contains("cargo test"), "the command is not named");
    assert!(
        md.contains("git checkout 4f2a9c1e"),
        "no re-derivation step"
    );
    assert!(md.contains("Check this yourself"), "no instructions at all");
}

#[test]
fn both_shapes_report_the_same_commit_and_the_same_outcome() {
    let w = passing();
    let c = Certificate::of(&w, None);
    let j = c.json();
    assert_eq!(j["subject"][0]["digest"]["gitCommit"], "4f2a9c1e");
    assert_eq!(
        j["predicate"]["evidence"]["commands"][0]["outcome"]["code"],
        0
    );
    assert!(c.markdown().contains("exit 0"));
}

#[test]
fn the_structured_shape_is_an_in_toto_statement() {
    // A shape supply-chain tooling already parses.
    let w = passing();
    let j = Certificate::of(&w, None).json();
    assert_eq!(j["_type"], "https://in-toto.io/Statement/v1");
    assert_eq!(
        j["predicateType"],
        "https://devplane.dev/DoneCertificate/v1"
    );
    assert!(j["subject"].is_array());
    assert!(j["predicate"].is_object());
}

#[test]
fn it_says_it_is_unsigned_rather_than_implying_otherwise() {
    // Unsigned: a signature would invite trusting the builder.
    let w = passing();
    assert_eq!(
        Certificate::of(&w, None).json()["predicate"]["signed"],
        false
    );
}

#[test]
fn the_verification_steps_are_data_and_not_only_prose() {
    let w = passing();
    let j = Certificate::of(&w, None).json();
    let steps = j["predicate"]["verification"]["steps"].as_array().unwrap();
    assert!(steps[0].as_str().unwrap().starts_with("git clone "));
    assert_eq!(steps[1], "git checkout 4f2a9c1e");
    assert_eq!(steps[2], "cargo test");
}

/// The certificate names the repository and a clone step, not just a commit.
#[test]
fn the_certificate_says_which_repository() {
    let w = passing();
    let c = Certificate::of(&w, None);
    let md = c.markdown();
    assert!(md.contains("acme/widgets"), "no repository is named:\n{md}");
    assert!(
        c.verification_steps()
            .iter()
            .any(|s| s.contains("git clone")),
        "the steps say what to check out but not where to get it"
    );
}

#[test]
fn the_attempt_says_which_of_how_many() {
    let mut w = change();
    for a in 1..=4 {
        let code = if a == 4 { 0 } else { 1 };
        w.gates.push(report(
            "check",
            a,
            vec![CommandResult::exited("cargo test", code)],
        ));
    }
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(
        md.contains("attempt 4 of 4"),
        "\"passed on the fourth try\" and \"passed\" are different sentences:\n{md}"
    );
}

#[test]
fn the_evidence_is_the_run_the_basis_names_not_merely_the_last_one() {
    let mut w = change();
    w.gates
        .push(report("check", 1, vec![CommandResult::exited("first", 0)]));
    w.gates
        .push(report("check", 2, vec![CommandResult::exited("second", 1)]));
    // A basis pointing at attempt 1 must show attempt 1's commands.
    let w = finished(
        w,
        Completion::GatesPassed {
            gate: "check".into(),
            attempt: 1,
            attempts: 2,
            at: jiff::Timestamp::now(),
        },
    );
    let md = Certificate::of(&w, None).markdown();
    assert!(
        md.contains("first"),
        "showed a different run from the basis"
    );
    assert!(!md.contains("second"));
}

// ── Absence never reads as zero ────────────────────────────────────────────

#[test]
fn a_gate_that_ran_outside_a_repository_says_so() {
    let mut w = change();
    let mut r = report("check", 1, vec![CommandResult::exited("cargo test", 0)]);
    r.commit = None;
    w.gates.push(r);
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("No commit was recorded"), "{md}");
    assert!(
        !md.contains("clean"),
        "an absent commit must not read as clean"
    );
}

#[test]
fn a_repository_with_no_commits_yet_is_a_state_not_a_blank() {
    let mut w = change();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)]);
    r.commit = Some(stamp(None, true, Reach::NoRemote));
    w.gates.push(r);
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let c = Certificate::of(&w, None);
    assert!(c.markdown().contains("had no commits yet"));
    assert!(c.verification_caveat().is_some());
}

#[test]
fn a_commit_nobody_can_fetch_says_so_where_the_instructions_are() {
    // Instructions to check out an unfetchable commit are worse than none.
    let mut w = change();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)]);
    r.commit = Some(stamp(Some("deadbeef"), true, Reach::LocalOnly));
    w.gates.push(r);
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    let instructions = md.find("Check this yourself").expect("no instructions");
    let warning = md.find("on no remote").expect("no local-only warning");
    assert!(
        warning > instructions - 400,
        "the warning is nowhere near the instructions it qualifies"
    );
    assert!(md.contains("cannot fetch it"), "{md}");
}

#[test]
fn the_four_reaches_read_differently() {
    let mut seen = std::collections::HashSet::new();
    for reach in [
        Reach::Remote,
        Reach::LocalOnly,
        Reach::NoRemote,
        Reach::Unknown,
    ] {
        let mut w = change();
        let mut r = report("check", 1, vec![CommandResult::exited("x", 0)]);
        r.commit = Some(stamp(Some("abc"), true, reach));
        w.gates.push(r);
        let basis = Completion::of(
            &w,
            true,
            Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
        );
        let w = finished(w, basis);
        assert!(
            seen.insert(Certificate::of(&w, None).markdown()),
            "two reachability states produced the same document"
        );
    }
}

#[test]
fn a_dirty_tree_is_said_where_the_commit_is() {
    let mut w = change();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)]);
    r.commit = Some(stamp(Some("abc123"), false, Reach::Remote));
    w.gates.push(r);
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    // Where the commit is shown; a stale-pass headline also mentions it.
    let at_commit = md.find("Commit `abc123`").expect("the commit is shown");
    let warning = md.find("does NOT describe").expect("no dirty warning");
    assert!(
        warning - at_commit < 300,
        "a reviewer who reads the commit has left the page before a footnote arrives"
    );
}

#[test]
fn the_four_outcomes_produce_four_sentences() {
    let mut seen = std::collections::HashSet::new();
    for outcome in [
        Outcome::Exited { code: 1 },
        Outcome::TimedOut { after_secs: 600 },
        Outcome::NeverStarted {
            reason: "no such file".into(),
        },
        Outcome::Unknown {
            reason: "killed by a signal".into(),
        },
    ] {
        let mut w = change();
        w.gates.push(report(
            "check",
            1,
            vec![CommandResult::without_verdict("cargo test", outcome)],
        ));
        let basis = Completion::of(
            &w,
            true,
            Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
        );
        let w = finished(w, basis);
        assert!(
            seen.insert(Certificate::of(&w, None).markdown()),
            "two outcomes rendered alike"
        );
    }
}

#[test]
fn a_specification_that_was_not_found_is_stated_not_omitted() {
    let mut w = change();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)]);
    r.spec = Some(SpecStamp {
        path: "specs/reset/".into(),
        fingerprint: None,
        files: 0,
        tasks_done: 0,
        tasks_total: 0,
        open_questions: 0,
    });
    w.gates.push(r);
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("there was no such file or folder"), "{md}");
}

#[test]
fn unticked_tasks_appear_beside_the_outcome_and_neither_is_judged() {
    let mut w = change();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)]);
    r.spec = Some(SpecStamp {
        path: "specs/012/".into(),
        fingerprint: Some("3f9a1c05e7b2d648".into()),
        files: 4,
        tasks_done: 28,
        tasks_total: 31,
        open_questions: 1,
    });
    w.gates.push(r);
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("28 of 31 ticked"));
    assert!(md.contains("exit 0"));
    assert!(
        md.contains("you decide"),
        "the tool must not resolve the contradiction"
    );
    for banned in ["consistent", "matches", "agrees", "score"] {
        assert!(
            !md.contains(banned),
            "the tool graded the two against each other: {banned}"
        );
    }
}

// ── Every route to done says what it rests on ──────────────────────────────

#[test]
fn the_four_bases_read_differently_one_at_a_time() {
    let at = jiff::Timestamp::now();
    let bases = [
        Completion::GatesPassed {
            gate: "check".into(),
            attempt: 1,
            attempts: 1,
            at,
        },
        Completion::GatesStale {
            gate: "check".into(),
            attempt: 1,
            attempts: 1,
            ran_at: Some(Box::new(stamp(Some("4f2a9c1e"), true, Reach::Remote))),
            now: Some(Box::new(stamp(Some("b7c2ffff"), true, Reach::Remote))),
            at,
        },
        Completion::NoGateDeclared { at },
        Completion::ByHand {
            at,
            last_gate: Some("check failed: cargo test".into()),
        },
    ];
    let mut seen = std::collections::HashSet::new();
    for b in &bases {
        assert!(seen.insert(b.headline()), "two bases share a headline");
    }
    // And the three that are not checks must not read as passes.
    assert!(bases[2].headline().contains("nothing was checked"));
    assert!(bases[3].headline().contains("did not pass"));
    for b in &bases[1..] {
        assert!(!b.is_checked(), "{}", b.headline());
        assert!(b.unchecked_note().is_some(), "{}", b.headline());
    }
    for b in &bases[..1] {
        assert!(b.is_checked());
        assert!(b.unchecked_note().is_none());
    }
    // A stale pass says the gates passed and what changed, never that they failed.
    let stale = bases[1].headline();
    assert!(stale.contains("the gates passed"), "{stale}");
    assert!(stale.contains("changed after"), "{stale}");
    assert!(!stale.contains("did not pass"), "{stale}");
    assert!(
        !bases[1]
            .unchecked_note()
            .unwrap()
            .contains("Nothing was checked"),
        "a stale pass is not *nothing was checked*"
    );
}

#[test]
fn a_project_with_no_gates_is_not_reported_as_a_pass() {
    let w = change();
    let basis = Completion::of(&w, false, None);
    assert!(matches!(basis, Completion::NoGateDeclared { .. }));
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("never said what done means"), "{md}");
    assert!(md.contains("Nothing was checked by this tool"), "{md}");
}

#[test]
fn a_failing_gate_cannot_produce_a_gates_passed_basis() {
    let mut w = change();
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("cargo test", 101)],
    ));
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    assert!(
        matches!(basis, Completion::ByHand { .. }),
        "a failing gate produced a passing basis"
    );
    assert!(basis.headline().contains("finished by hand"));
}

#[test]
fn no_basis_names_a_person_because_there_is_no_identity_to_name() {
    let at = jiff::Timestamp::now();
    let j = serde_json::to_value(Completion::ByHand {
        at,
        last_gate: None,
    })
    .unwrap();
    let obj = j.as_object().unwrap();
    for k in ["who", "user", "actor", "author", "by"] {
        assert!(
            !obj.contains_key(k),
            "a field this product cannot fill truthfully: {k}"
        );
    }
}

// ── What it refuses to claim ───────────────────────────────────────────────

#[test]
fn the_limits_travel_with_the_paste() {
    let w = passing();
    let md = Certificate::of(&w, None).markdown();
    assert!(
        md.contains(LIMITS),
        "the statement of limits is not in the artifact"
    );
    assert!(md.contains("not evidence that the work is correct"));
}

#[test]
fn every_rederivable_field_is_named_in_both_shapes_from_one_list() {
    let w = passing();
    let c = Certificate::of(&w, None);
    let md = c.markdown();
    let j = c.json();
    let listed = j["predicate"]["rederivable"].as_array().unwrap();
    assert_eq!(listed.len(), REDERIVABLE.len());
    for f in REDERIVABLE {
        assert!(
            md.contains(f),
            "the document omits a re-derivable field: {f}"
        );
        assert!(listed.iter().any(|v| v == f));
    }
}

#[test]
fn it_does_not_invite_the_reader_to_compare_digests() {
    let w = passing();
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("not reproducible"), "{md}");
    assert!(md.contains("bind the stored output to the run"));
}

#[test]
fn the_agent_claim_never_appears_without_an_outcome_beside_it() {
    let w = passing();
    let md = Certificate::of(&w, Some("I fixed everything and all tests pass.")).markdown();
    let claim = md.find("I fixed everything").expect("the claim is missing");
    let outcome = md.find("exit 0").expect("no outcome anywhere");
    assert!(outcome < claim, "the claim precedes any evidence");
    assert!(md.contains("agent's own account"));
    assert!(md.contains("one action in eleven"));
}

#[test]
fn a_change_with_no_claim_is_not_reported_as_an_agent_that_said_nothing() {
    let w = passing();
    let md = Certificate::of(&w, None).markdown();
    assert!(
        !md.contains("The agent's claim"),
        "an absent claim grew a section"
    );
}

// ── The awkward shapes ─────────────────────────────────────────────────────

#[test]
fn an_unfinished_change_exports_an_honest_account_rather_than_a_certificate() {
    let mut w = change();
    w.runs.push(devplane::core::RunId::new("r1"));
    w.waiting = Some(Waiting::Gates);
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("cargo test", 101)],
    ));
    let c = Certificate::of(&w, None);
    assert!(!c.is_finished());
    let md = c.markdown();
    assert!(md.contains("not finished"), "{md}");
    assert!(
        md.contains("in flight") && md.contains("gates running"),
        "it does not say where the change is: {md}"
    );
    assert!(md.contains("Its last gate said"));
    assert!(
        !md.contains("Check this yourself"),
        "it offered instructions anyway"
    );
}

#[test]
fn the_artifact_is_bounded_and_says_that_it_truncated() {
    let mut w = change();
    let huge = CommandResult {
        output_tail: "x".repeat(200_000),
        output_bytes: 100 * 1024 * 1024,
        ..CommandResult::exited("cargo test", 0)
    };
    w.gates.push(report("check", 1, vec![huge]));
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown_bounded();
    assert!(md.len() <= MAX_BYTES, "{} bytes", md.len());
    assert!(
        md.contains("104857600 bytes"),
        "it does not say how much there was"
    );
}

#[test]
fn untrusted_text_cannot_restructure_the_document() {
    // Commands and titles are untrusted; Markdown has no serialiser to escape them.
    let nasty = "```\n# Fake heading\n| a | b |\n<!-- comment -->\n```";
    let mut w = change();
    w.title = nasty.into();
    w.gates
        .push(report("check", 1, vec![CommandResult::exited(nasty, 0)]));
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let c = Certificate::of(&w, None);
    let md = c.markdown();

    for section in [
        "# Done certificate",
        "## What was checked",
        "## Check this yourself",
        "## What this does not establish",
    ] {
        assert!(
            md.contains(section),
            "untrusted text ate a section: {section}"
        );
    }
    // Inside a fence the payload is literal text, so check where lines are,
    // not whether they exist.
    for (i, line) in outside_fences(&md) {
        assert!(
            !line.trim_start().starts_with('#')
                || line.contains("Done certificate")
                || line.starts_with("## "),
            "line {i} is markup outside any fence: {line}"
        );
        assert!(
            !line.trim_start().starts_with("| a | b |"),
            "an injected table row opened a row of the document at line {i}"
        );
    }
    assert!(md.contains("Fake heading"));
    assert_eq!(c.json()["predicate"]["change"]["title"], nasty);
}

/// Numbered lines outside fenced blocks; a fence closes on a run of equal or
/// greater length.
fn outside_fences(md: &str) -> Vec<(usize, &str)> {
    let mut open: Option<usize> = None;
    let mut out = Vec::new();
    for (i, line) in md.lines().enumerate() {
        let ticks = line.len() - line.trim_start_matches('`').len();
        match open {
            Some(n) if ticks >= n && line.trim() == "`".repeat(ticks) => open = None,
            Some(_) => {}
            None if ticks >= 3 && line.trim() == "`".repeat(ticks) => open = Some(ticks),
            None => out.push((i + 1, line)),
        }
    }
    out
}

#[test]
fn a_fence_in_the_verification_block_cannot_close_it() {
    let mut w = change();
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("echo '```'; rm -rf /", 0)],
    ));
    let basis = Completion::of(
        &w,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    let start = md.find("## Check this yourself").unwrap();
    let block = &md[start..];
    let fence_line = block
        .lines()
        .find(|l| l.starts_with("```"))
        .expect("no fence");
    assert!(
        fence_line.len() > 3,
        "the fence is no longer than the content it must contain"
    );
    assert!(
        md.contains("## What this does not establish"),
        "the block escaped"
    );
}

// ── The invariants that make the basis worth reading ───────────────────────

/// A completion is assigned in exactly one place in `src/`; the compiler
/// cannot notice a second direct assignment.
#[test]
fn done_is_written_in_exactly_one_place() {
    let mut sites = Vec::new();
    for entry in std::fs::read_dir("src").unwrap() {
        let mut stack = vec![entry.unwrap().path()];
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                stack.extend(std::fs::read_dir(&p).unwrap().map(|e| e.unwrap().path()));
                continue;
            }
            if p.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            for (i, line) in text.lines().enumerate() {
                // An assignment, not a comparison or a match arm.
                if line.contains(".completion = Some(") {
                    sites.push(format!("{}:{}", p.display(), i + 1));
                }
            }
        }
    }
    assert_eq!(
        sites.len(),
        1,
        "the one transition that makes this product's headline claim now has \
         {} writers: {sites:?}",
        sites.len()
    );
    assert!(sites[0].contains("change.rs"), "{sites:?}");
}

/// The waiting setter has no way to say *finished* or *verified*.
#[test]
fn the_waiting_setter_cannot_express_done() {
    let src = std::fs::read_to_string("src/core/change.rs").unwrap();
    let waiting = src
        .split("pub enum Waiting {")
        .nth(1)
        .and_then(|s| s.split("\n}").next())
        .expect("the Waiting enum is gone, and with it the barrier");
    for word in ["Finished", "Done", "Verified"] {
        assert!(
            !waiting.contains(word),
            "`{word}` was added to what a change can wait on, so a caller can mark a \
             change finished without recording what it rests on"
        );
    }
    let setter = std::fs::read_to_string("src/change.rs").unwrap();
    assert!(
        setter.contains("pub(crate) async fn set_waiting(state: &Shared, id: &ChangeId, waiting: Option<Waiting>)"),
        "the one setter takes something other than what a change waits on"
    );
}

/// Every finished change has a basis, and no unfinished one does.
#[test]
fn a_basis_exists_exactly_when_the_change_is_finished() {
    let unfinished = change();
    assert!(unfinished.completion.is_none());
    assert!(!Certificate::of(&unfinished, None).is_finished());

    let done = passing();
    assert!(Certificate::of(&done, None).is_finished());
    assert!(
        done.completion.is_some(),
        "a finished work with nothing attached"
    );
}

/// The basis constructor is total: it answers for every shape of change.
#[test]
fn every_shape_of_change_gets_a_basis() {
    let mut cases = vec![change()];

    let mut passed = change();
    passed
        .gates
        .push(report("check", 1, vec![CommandResult::exited("x", 0)]));
    cases.push(passed);

    let mut failed = change();
    failed
        .gates
        .push(report("check", 1, vec![CommandResult::exited("x", 1)]));
    cases.push(failed);

    let mut hung = change();
    hung.gates.push(report(
        "check",
        1,
        vec![CommandResult::without_verdict(
            "x",
            Outcome::TimedOut { after_secs: 1 },
        )],
    ));
    cases.push(hung);

    let here = stamp(Some("4f2a9c1e"), true, Reach::Remote);
    for w in &cases {
        for declares in [true, false] {
            for now in [None, Some(&here)] {
                let basis = Completion::of(w, declares, now);
                assert!(!basis.headline().is_empty());
            }
        }
    }
}

/// A pass is computed against the current tree: once the commit or working
/// tree moves on, or cannot be read, it becomes stale with no flag to clear.
#[test]
fn a_gate_that_passed_on_a_commit_that_moved_on_is_no_longer_a_pass() {
    let w = passing();
    let here = stamp(Some("4f2a9c1e"), true, Reach::Remote);

    assert!(
        matches!(
            Completion::of(&w, true, Some(&here)),
            Completion::GatesPassed { .. }
        ),
        "a gate on the very commit it ran against is current"
    );

    // The branch moved on: stale, not failed.
    let moved = stamp(Some("b7c2ffff"), true, Reach::Remote);
    let basis = Completion::of(&w, true, Some(&moved));
    assert!(
        matches!(basis, Completion::GatesStale { .. }),
        "a commit that moved on must stop being a pass: {basis:?}"
    );
    assert!(!basis.is_checked());
    let said = basis.headline();
    assert!(
        said.contains("tree-of") && said.contains("changed after"),
        "{said}"
    );
    assert!(!said.contains("did not pass"), "{said}");

    // Dirty alone is not stale: the same working-tree digest is the same tree.
    let dirty = stamp(Some("4f2a9c1e"), false, Reach::Remote);
    assert!(matches!(
        Completion::of(&w, true, Some(&dirty)),
        Completion::GatesPassed { .. }
    ));

    // Same commit, different working-tree digest.
    let mut touched = dirty.clone();
    touched.tree = Some("0dd5ea7e".into());
    let basis = Completion::of(&w, true, Some(&touched));
    assert!(
        matches!(basis, Completion::GatesStale { .. }),
        "touching one file makes it false"
    );
    assert!(
        basis.headline().contains("0dd5ea7"),
        "it names the tree now: {}",
        basis.headline()
    );

    // Git could not answer: unknown is not unchanged.
    let basis = Completion::of(&w, true, None);
    assert!(
        matches!(basis, Completion::GatesStale { now: None, .. }),
        "a tree that cannot be read is not a verified one"
    );
    assert!(
        basis.headline().contains("could not be read"),
        "{}",
        basis.headline()
    );

    let done = finished(w.clone(), basis);
    let md = Certificate::of(&done, None).markdown();
    assert!(md.contains("cargo test"), "{md}");
    assert!(md.contains("not against the tree as it stands"), "{md}");
    assert!(!md.contains("Nothing was checked"), "{md}");
}

// ── Against real git, because the claims are about somebody else's tool ─────

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dp-cert-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test]
async fn a_repository_with_no_commits_reports_no_commit_rather_than_failing() {
    let dir = scratch("initial");
    git(&dir, &["init", "-q", "-b", "main"]);
    let stamp = devplane::git::commit_stamp(&dir)
        .await
        .expect("no stamp at all");
    assert!(
        stamp.commit.is_none(),
        "`(initial)` was stored as if it were a commit"
    );
    assert!(stamp.clean);
}

#[tokio::test]
async fn a_commit_with_no_remote_is_not_reported_as_unpushed() {
    // No remote at all is a different state from a remote that lacks it.
    let dir = scratch("noremote");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "one"]);
    let stamp = devplane::git::commit_stamp(&dir).await.unwrap();
    assert!(stamp.commit.is_some());
    assert_eq!(stamp.reach, Reach::NoRemote);
    assert!(!stamp.checkable_by_others());
}

#[tokio::test]
async fn a_commit_that_is_only_local_says_so_and_a_pushed_one_does_not() {
    let origin = scratch("origin");
    git(&origin, &["init", "-q", "--bare", "-b", "main"]);
    let dir = scratch("clone");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "one"]);

    let before = devplane::git::commit_stamp(&dir).await.unwrap();
    assert_eq!(
        before.reach,
        Reach::LocalOnly,
        "an unpushed commit read as fetchable"
    );
    assert!(!before.checkable_by_others());

    git(&dir, &["push", "-q", "origin", "main"]);
    let after = devplane::git::commit_stamp(&dir).await.unwrap();
    assert_eq!(after.reach, Reach::Remote);
    assert!(after.checkable_by_others());
    assert_eq!(
        after.commit, before.commit,
        "the commit itself did not change"
    );
}

#[tokio::test]
async fn a_dirty_tree_is_recorded_with_the_commit_from_one_observation() {
    let dir = scratch("dirty");
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "one").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    assert!(devplane::git::commit_stamp(&dir).await.unwrap().clean);

    std::fs::write(dir.join("b.txt"), "two").unwrap();
    let dirty = devplane::git::commit_stamp(&dir).await.unwrap();
    assert!(
        !dirty.clean,
        "an untracked file left the tree reported as clean"
    );
    assert_eq!(dirty.changed_files, 1);
    assert!(
        dirty.commit.is_some(),
        "the commit is still true, just incomplete"
    );
}

#[tokio::test]
async fn outside_a_repository_there_is_no_stamp_at_all() {
    let dir = scratch("norepo");
    assert!(devplane::git::commit_stamp(&dir).await.is_none());
}

/// Evidence and claim carry an origin (`externally_observed` / `self_reported`,
/// per the proposed OpenTelemetry GenAI attribute) in both shapes, and an
/// absent one is never defaulted.
#[test]
fn the_certificate_says_where_its_predicate_came_from_and_never_defaults_it() {
    use devplane::core::certificate::Origin;

    let key = Origin::KEY;

    let w = passing();
    let cert = Certificate::of(&w, Some("I added rate limiting and it all works."));
    let j = cert.json();
    let predicate = &j["predicate"];

    assert_eq!(
        predicate["evidence"][key], "externally_observed",
        "a gate transcript is the one thing here that is not the agent's word"
    );
    assert_eq!(
        predicate["agent_claim"][key], "self_reported",
        "the agent's account has to say that is what it is"
    );
    assert_eq!(
        predicate["agent_claim"]["text"], "I added rate limiting and it all works.",
        "the claim itself still travels"
    );

    let md = cert.markdown();
    assert!(md.contains(&format!("{key}: externally_observed")), "{md}");
    assert!(md.contains(&format!("{key}: self_reported")), "{md}");

    // No gate and no claim: no origin at all.
    let bare = finished(
        change(),
        Completion::NoGateDeclared {
            at: jiff::Timestamp::now(),
        },
    );
    let none = Certificate::of(&bare, None);
    let jn = none.json();
    assert!(
        jn["predicate"]["evidence"].is_null(),
        "no gate ran, so there is no evidence to carry an origin"
    );
    assert!(
        jn["predicate"]["agent_claim"].is_null(),
        "the agent said nothing, which is not the same as saying something unverified"
    );
    assert!(
        !none.markdown().contains(key),
        "an origin appeared on a certificate that has no evidence and no claim"
    );
}

/// Each of the four completions (passed, stale, no gate declared, by hand)
/// renders a distinct basis; *no gate declared* is a stated warning, not a blank.
#[test]
fn every_way_a_change_reaches_done_says_which_one_it_was() {
    let at = jiff::Timestamp::now();

    let stale = {
        let w = passing();
        let moved = stamp(Some("b7c2ffff"), true, Reach::Remote);
        let basis = Completion::of(&w, true, Some(&moved));
        finished(w, basis)
    };
    let cases: Vec<(&str, Change)> = vec![
        ("gates passed", passing()),
        ("gates stale", stale),
        (
            "no gate declared",
            finished(change(), Completion::NoGateDeclared { at }),
        ),
        (
            "by hand",
            finished(
                change(),
                Completion::ByHand {
                    at,
                    last_gate: None,
                },
            ),
        ),
    ];

    for (name, w) in &cases {
        let page = Certificate::of(w, None).page();
        assert_eq!(page["finished"], true, "{name}");

        let basis = page["basis"].as_str().unwrap_or_default();
        assert!(!basis.is_empty(), "{name} renders no basis at all");

        if *name == "no gate declared" {
            assert!(
                basis.contains("never said what done means"),
                "an empty evidence block is what this used to render: {basis}"
            );
            assert_eq!(page["checked"], false);
            assert!(
                page["unchecked"].is_string(),
                "an unchecked completion must carry its warning"
            );
        }
        if *name == "gates passed" {
            assert_eq!(page["checked"], true, "{name}");
            assert!(
                page["unchecked"].is_null(),
                "{name} is checked, so nothing warns about it"
            );
        }
        if *name == "gates stale" {
            assert_eq!(page["checked"], false);
            let warned = page["unchecked"].as_str().expect("a warning");
            assert!(
                warned.contains("not against the tree as it stands"),
                "{warned}"
            );
            assert!(page["evidence"].is_object(), "{page}");
        }
    }

    // By hand: says no check ran rather than leaving evidence blank.
    let by_hand = finished(
        change(),
        Completion::ByHand {
            at,
            last_gate: None,
        },
    );
    let page = Certificate::of(&by_hand, None).page();
    assert!(
        page["basis"]
            .as_str()
            .unwrap_or_default()
            .contains("finished by hand"),
        "{:?}",
        page["basis"]
    );
    assert!(
        page["no_evidence"].is_string(),
        "no gate ran and nothing says so"
    );

    let said: std::collections::BTreeSet<String> = cases
        .iter()
        .map(|(_, w)| {
            Certificate::of(w, None).page()["basis"]
                .as_str()
                .unwrap_or("")
                .to_string()
        })
        .collect();
    assert_eq!(said.len(), 4, "two completions read the same: {said:?}");
}

/// Both shapes carry git's tree digest beside the commit, so a reviewer can
/// re-derive it with `git rev-parse HEAD^{tree}`.
#[test]
fn the_certificate_carries_the_tree_digest_beside_the_commit() {
    let w = passing();
    let c = Certificate::of(&w, None);
    let md = c.markdown();
    let at_commit = md.find("4f2a9c1e").expect("the commit is named");
    let at_tree = md
        .find("tree-of-4f2a9c1e")
        .expect("the tree digest is not on the certificate");
    assert!(
        at_tree.abs_diff(at_commit) < 80,
        "the tree is nowhere near the commit it belongs to"
    );
    assert_eq!(
        c.json()["predicate"]["evidence"]["commit"]["tree"],
        "tree-of-4f2a9c1e"
    );
    assert!(
        REDERIVABLE.contains(&"evidence.commit.tree"),
        "a digest git produces is one a reader can re-derive"
    );

    // A stamp with no tree says so rather than showing a blank.
    let mut w = change();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)]);
    let mut old = stamp(Some("abc123"), true, Reach::Remote);
    old.tree = None;
    r.commit = Some(old);
    w.gates.push(r);
    let basis = Completion::of(&w, true, Some(&stamp(Some("abc123"), true, Reach::Remote)));
    let w = finished(w, basis);
    assert!(
        Certificate::of(&w, None)
            .markdown()
            .contains("tree digest not recorded")
    );
}

/// Every `--flag` in any rendering of the certificate exists in `src/cli/`.
#[test]
fn the_certificate_offers_no_flag_the_cli_does_not_have() {
    let w = passing();
    let c = Certificate::of(&w, None);
    // Past the size ceiling, so the truncation note's command is checked too.
    let mut huge = change();
    huge.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("x".repeat(2 * MAX_BYTES), 0)],
    ));
    let basis = Completion::of(
        &huge,
        true,
        Some(&stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    );
    let huge = finished(huge, basis);
    let corpus = [
        c.markdown(),
        c.page().to_string(),
        c.json().to_string(),
        Certificate::of(&huge, None).markdown_bounded(),
    ]
    .join("\n");

    let mut flags = std::collections::BTreeSet::new();
    let mut rest = corpus.as_str();
    while let Some(i) = rest.find("--") {
        let after = &rest[i + 2..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '-')
            .collect();
        if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
            flags.insert(format!("--{name}"));
        }
        rest = after;
    }
    assert!(
        !flags.is_empty(),
        "the certificate mentions no flag at all, so this guard compares nothing"
    );

    let mut cli = String::new();
    for entry in std::fs::read_dir("src/cli").expect("src/cli") {
        let p = entry.unwrap().path();
        if p.extension().is_some_and(|e| e == "rs") {
            cli.push_str(&std::fs::read_to_string(&p).unwrap());
        }
    }
    for flag in &flags {
        assert!(
            cli.contains(flag),
            "the certificate offers `{flag}`, which no command in src/cli/ has"
        );
    }
    assert!(
        !corpus.contains("--sign"),
        "`--sign` is offered again, and there is no such flag"
    );
}

/// The page truncates a long command to a prefix; the copy carries it whole
/// and pasteable.
#[test]
fn a_long_command_is_short_on_the_page_and_complete_in_the_copy() {
    let long = format!(
        "cargo test {} -- --nocapture",
        "-p some-very-long-crate-name ".repeat(12)
    );
    let mut w = change();
    w.gates
        .push(report("check", 1, vec![CommandResult::exited(&long, 0)]));
    let w = finished(
        w,
        Completion::GatesPassed {
            gate: "check".into(),
            attempt: 1,
            attempts: 1,
            at: jiff::Timestamp::now(),
        },
    );

    let cert = Certificate::of(&w, None);
    let page = cert.page();
    let cmd = &page["evidence"]["commands"][0];

    assert_eq!(cmd["command"], long, "the full command has to travel");
    assert_eq!(cmd["truncated"], true);

    let shown = cmd["shown"].as_str().expect("a shown form");
    assert!(
        shown.chars().count() < long.chars().count(),
        "nothing was truncated"
    );

    let head: String = shown.chars().take_while(|c| *c != '…').collect();
    assert!(
        long.starts_with(head.trim_end()),
        "the shown command is not a prefix of the real one:\n  shown: {shown}\n  real:  {long}"
    );

    assert!(cert.markdown().contains(&long), "the copy lost the command");
}

/// The surface's plan and the certificate's stamp give the same, correct
/// figures for one folder: only `tasks.md` boxes count, and `checklists/`,
/// fenced boxes and backticked markers do not.
#[test]
fn the_surface_and_the_certificate_count_one_plan_the_same_way() {
    let root = std::env::temp_dir().join(format!("dp-plan-{}", uuid::Uuid::new_v4().simple()));
    let dir = root.join("specs/001-feature");
    std::fs::create_dir_all(dir.join("checklists")).unwrap();
    std::fs::write(
        dir.join("spec.md"),
        "# Feature\n\n## Why\n\nIt collects `[NEEDS CLARIFICATION]` lines.\n\n\
         ```\n- [x] a fenced example box\n```\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("tasks.md"),
        "# Tasks\n\n- [x] T001 done\n- [x] T002 done\n- [ ] T003 open\n- [ ] T004 open\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("plan.md"),
        "# Plan\n\nHow long may it wait? [NEEDS CLARIFICATION]\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("checklists/requirements.md"),
        "# Checklist\n\n- [x] No [NEEDS CLARIFICATION] markers remain\n- [ ] unticked row\n",
    )
    .unwrap();

    let markers = vec!["NEEDS CLARIFICATION".to_string()];
    let plan = devplane::core::spec::Plan::read(&root, "specs/001-feature", &markers);
    let stamp = devplane::core::change::SpecStamp::of("specs/001-feature", &root, &markers);

    assert_eq!(
        plan.progress.map(|p| (p.done, p.total)),
        stamp.tasks(),
        "the surface and the certificate disagree about the boxes"
    );
    assert_eq!(plan.open_questions, stamp.open_questions);
    assert_eq!(plan.files, stamp.files);
    assert_eq!(plan.fingerprint, stamp.fingerprint);

    assert_eq!(
        plan.progress.map(|p| (p.done, p.total)),
        Some((2, 4)),
        "`tasks.md` only: the checklist's boxes and the fenced example are not progress"
    );
    assert_eq!(
        plan.open_questions, 1,
        "one real marker; the checklist row and the backticked mention are not questions"
    );
    assert!(plan.present);
    assert!(
        plan.contradicts_done(),
        "two boxes open and a question unanswered is a contradiction with done"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A plan with no boxes has no progress (`Progress::of(0, 0)` is `None`), so it
/// cannot render as complete.
#[test]
fn a_plan_with_no_boxes_has_no_progress_to_render() {
    assert_eq!(devplane::core::spec::Progress::of(0, 0), None);
    let p = devplane::core::spec::Progress::of(2, 4).expect("boxes exist");
    assert_eq!(p.open(), 2);
    assert!(!p.complete());
    assert!(devplane::core::spec::Progress::of(4, 4).unwrap().complete());
}

/// A change naming a specification that is not there reads as a
/// contradiction with done.
#[test]
fn a_plan_that_is_not_there_is_a_finding_rather_than_a_blank() {
    let root = std::env::temp_dir().join(format!("dp-noplan-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&root).unwrap();
    let plan = devplane::core::spec::Plan::read(&root, "specs/never-written", &[]);
    assert!(!plan.present);
    assert_eq!(plan.files, 0);
    assert_eq!(plan.progress, None);
    assert!(
        plan.contradicts_done(),
        "done against a specification that does not exist is the clearest contradiction there is"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A project that declares no markers gets no questions: the vocabulary is the
/// repository's, so there is no default list.
#[test]
fn a_project_that_declares_no_markers_has_no_questions() {
    let root = std::env::temp_dir().join(format!("dp-nomark-{}", uuid::Uuid::new_v4().simple()));
    let dir = root.join("specs/001-x");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("spec.md"),
        "# X\n\nSomething [NEEDS CLARIFICATION] here, and TODO and ??? too.\n",
    )
    .unwrap();

    let plan = devplane::core::spec::Plan::read(&root, "specs/001-x", &[]);
    assert_eq!(
        plan.open_questions, 0,
        "a word nobody declared was collected"
    );
    assert!(plan.questions.is_empty());

    let id = devplane::core::ProjectId::from("p1");
    assert!(
        devplane::core::attention::plan_question_item_at(
            "proj",
            &id,
            &[("w1".into(), plan)],
            jiff::Timestamp::UNIX_EPOCH,
        )
        .is_none(),
        "an item was raised for a project that declared no words"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Many markers in one project raise one inbox item naming the count, never
/// one per marker.
#[test]
fn many_markers_in_one_project_are_one_item() {
    let root = std::env::temp_dir().join(format!("dp-manymark-{}", uuid::Uuid::new_v4().simple()));
    let dir = root.join("specs/001-x");
    std::fs::create_dir_all(&dir).unwrap();
    let body: String = (0..40)
        .map(|i| format!("- question {i} [NEEDS CLARIFICATION]\n"))
        .collect();
    std::fs::write(dir.join("spec.md"), format!("# X\n\n{body}")).unwrap();

    let markers = vec!["NEEDS CLARIFICATION".to_string()];
    let plan = devplane::core::spec::Plan::read(&root, "specs/001-x", &markers);
    assert_eq!(plan.open_questions, 40, "the count is the whole folder's");
    assert!(
        plan.questions.len() <= 20,
        "the lines handed to a surface are bounded; the count is not"
    );

    let id = devplane::core::ProjectId::from("p1");
    let item = devplane::core::attention::plan_question_item_at(
        "proj",
        &id,
        &[("w1".into(), plan)],
        jiff::Timestamp::UNIX_EPOCH,
    )
    .expect("forty markers is a fact worth one item");
    assert!(
        item.title.contains("40 questions"),
        "the item names the count: {}",
        item.title
    );
    assert_eq!(item.level, devplane::core::attention::Level::Normal);
    assert!(
        item.actions.is_empty(),
        "nothing here can be answered from the inbox: the answer is an edit to a committed file"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A change detects that its plan's fingerprint moved since it started; any
/// document under the spec folder counts, not only `tasks.md`.
#[test]
fn a_plan_that_moved_under_the_change_is_on_the_certificate() {
    let root = std::env::temp_dir().join(format!("dp-drift-{}", uuid::Uuid::new_v4().simple()));
    let dir = root.join("specs/001-x");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("spec.md"), "# X\n\n## Why\n").unwrap();
    std::fs::write(dir.join("tasks.md"), "# Tasks\n\n- [x] T001 done\n").unwrap();

    let before = devplane::core::spec::Spec::read(&root, "specs/001-x", &[])
        .fingerprint()
        .expect("a plan has a fingerprint");

    let mut work = devplane::core::Change::new(
        devplane::core::ProjectId::from("p1"),
        "fix it".into(),
        "do the thing".into(),
    );
    work.spec = Some("specs/001-x".into());
    work.spec_at_start = Some(before.clone());

    assert_eq!(work.plan_drifted(Some(&before)), Some(false));

    std::fs::write(
        dir.join("spec.md"),
        "# X\n\n## Why\n\n## And another thing\n",
    )
    .unwrap();
    let after = devplane::core::spec::Spec::read(&root, "specs/001-x", &[])
        .fingerprint()
        .expect("still has one");
    assert_ne!(
        before, after,
        "the fixture did not actually change the plan"
    );
    assert_eq!(work.plan_drifted(Some(&after)), Some(true));

    let _ = std::fs::remove_dir_all(&root);
}

/// With no starting fingerprint, drift is unknown (`None`), not `false`.
#[test]
fn a_change_with_no_starting_fingerprint_reports_unknown_rather_than_unchanged() {
    let mut work = devplane::core::Change::new(
        devplane::core::ProjectId::from("p1"),
        "t".into(),
        "p".into(),
    );
    work.spec = Some("specs/001-x".into());
    assert_eq!(
        work.spec_at_start, None,
        "a change created today should record one when it names a spec"
    );
    assert_eq!(
        work.plan_drifted(Some("anything")),
        None,
        "a change with nothing recorded claimed its plan had held still"
    );
    assert_eq!(work.plan_drifted(None), None);
}

// ── What verified means ────────────────────────────────────────────────────

/// The clone line is pasted into a shell; the remote URL is somebody else's
/// bytes and must arrive as one quoted word.
#[test]
fn the_clone_line_quotes_the_remote() {
    let mut w = passing();
    let evil = "https://x.test/a.git; rm -rf ~ #'";
    if let Some(c) = w.gates[0].commit.as_mut() {
        c.remote = Some(evil.into());
    }
    let steps = Certificate::of(&w, None).verification_steps();
    let quoted = devplane::core::certificate::shell_quote(evil);
    assert_eq!(quoted, "'https://x.test/a.git; rm -rf ~ #'\\'''");
    assert_eq!(
        steps[0],
        format!("git clone {quoted} && cd \"$(basename {quoted} .git)\"")
    );
}

/// The basis names attempt 2; the record holds only attempt 1. The
/// certificate shows no evidence and says why — never attempt 1 in its place.
#[test]
fn a_missing_basis_run_is_said_and_never_replaced() {
    let mut w = passing();
    w.completion = Some(Completion::GatesPassed {
        gate: "check".into(),
        attempt: 2,
        attempts: 2,
        at: jiff::Timestamp::now(),
    });
    let c = Certificate::of(&w, None);
    assert!(c.evidence.is_none());
    assert!(c.basis_run_missing());
    let md = c.markdown();
    assert!(
        md.contains("attempt 2, and that run is no longer in the record"),
        "{md}"
    );
    assert!(
        !md.contains("| `cargo test` |"),
        "another run was shown: {md}"
    );
}

/// The digest is the working tree's, and the certificate says it leaves
/// ignored files out.
#[test]
fn the_certificate_says_the_digest_excludes_ignored_files() {
    let md = Certificate::of(&passing(), None).markdown();
    assert!(md.contains("ignored files excluded"), "{md}");
    assert!(md.contains("git add -A && git write-tree"), "{md}");
    assert!(md.contains("Gate commands digest"), "{md}");
}

/// One sentence names the checks the change itself altered; none altered is
/// said too, and an unreadable diff is not *none*.
#[test]
fn the_certificate_names_the_checks_the_change_altered() {
    use devplane::core::review::Weakened;
    let w = passing();
    let mut c = Certificate::of(&w, None);
    assert!(c.markdown().contains("could not be read"));
    c.weakened = Some(Vec::new());
    assert!(c.markdown().contains("altered none of its checks"));
    c.weakened = Some(vec![Weakened {
        path: "tests/login.rs".into(),
        why: "adds a skip marker `#[ignore`".into(),
    }]);
    let md = c.markdown();
    assert!(
        md.contains("This change itself altered the checks it was verified by: `tests/login.rs`"),
        "{md}"
    );
}

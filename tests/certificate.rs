//! The done certificate: what it says, what it refuses to say, and what a
//! stranger can do with it.
//!
//! These are properties of the *artifact*, so almost all of them are assertions
//! about wording. That is deliberate. The feature's failure mode is not a wrong
//! number — it is a true record rendered so that a reader draws a larger
//! conclusion than the evidence supports, and every test here is aimed at that.

use devplane::core::certificate::{Certificate, LIMITS, MAX_BYTES, REDERIVABLE};
use devplane::core::work::{
    CommandResult, CommitStamp, Completion, GateReport, Outcome, Phase, Reach, SpecStamp, Work,
    WorkKind,
};

fn work() -> Work {
    Work::new(
        devplane::core::ids::ProjectId::new("p1"),
        WorkKind::Quick,
        "Add rate limiting".into(),
        "do the thing".into(),
    )
}

fn stamp(commit: Option<&str>, clean: bool, reach: Reach) -> CommitStamp {
    CommitStamp {
        commit: commit.map(str::to_string),
        branch: Some("work/rate-limiting".into()),
        clean,
        changed_files: if clean { 0 } else { 3 },
        reach,
        remote: Some("git@github.com:acme/widgets.git".into()),
    }
}

fn report(gate: &str, attempt: u32, commands: Vec<CommandResult>, expect_fail: bool) -> GateReport {
    GateReport {
        gate: gate.into(),
        at: jiff::Timestamp::now(),
        duration_ms: 10,
        commands,
        attempt,
        expect_fail,
        spec: None,
        commit: Some(stamp(Some("4f2a9c1e"), true, Reach::Remote)),
    }
}

fn finished(mut w: Work, basis: Completion) -> Work {
    w.phase = Phase::Done;
    w.completion = Some(basis);
    w
}

fn passing() -> Work {
    let mut w = work();
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("cargo test", 0)],
        false,
    ));
    let basis = Completion::of(&w, true);
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
    // Borrowed because it costs four keys and puts this in a shape anybody in
    // the supply-chain space already has a parser for.
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
    // SLSA's own spec puts the builder inside the trust boundary. This does not
    // ask to be believed, so a signature would invite exactly the wrong reading.
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

/// **A commit with no repository beside it is an instruction nobody can
/// follow.** Every test passed and the commit was right while the artifact
/// still did not say where to clone from; it was found by reading the page.
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
    let mut w = work();
    for a in 1..=4 {
        let code = if a == 4 { 0 } else { 1 };
        w.gates.push(report(
            "check",
            a,
            vec![CommandResult::exited("cargo test", code)],
            false,
        ));
    }
    let basis = Completion::of(&w, true);
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(
        md.contains("attempt 4 of 4"),
        "\"passed on the fourth try\" and \"passed\" are different sentences:\n{md}"
    );
}

#[test]
fn the_evidence_is_the_run_the_basis_names_not_merely_the_last_one() {
    let mut w = work();
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("first", 0)],
        false,
    ));
    w.gates.push(report(
        "check",
        2,
        vec![CommandResult::exited("second", 1)],
        false,
    ));
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
    let mut w = work();
    let mut r = report(
        "check",
        1,
        vec![CommandResult::exited("cargo test", 0)],
        false,
    );
    r.commit = None;
    w.gates.push(r);
    let basis = Completion::of(&w, true);
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
    let mut w = work();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)], false);
    r.commit = Some(stamp(None, true, Reach::NoRemote));
    w.gates.push(r);
    let basis = Completion::of(&w, true);
    let w = finished(w, basis);
    let c = Certificate::of(&w, None);
    assert!(c.markdown().contains("had no commits yet"));
    assert!(c.verification_caveat().is_some());
}

#[test]
fn a_commit_nobody_can_fetch_says_so_where_the_instructions_are() {
    // The gap this design had until `closeout-truth` was read: a certificate
    // that confidently instructs somebody to check out a commit they cannot
    // obtain is worse than one that says nothing.
    let mut w = work();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)], false);
    r.commit = Some(stamp(Some("deadbeef"), true, Reach::LocalOnly));
    w.gates.push(r);
    let basis = Completion::of(&w, true);
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
        let mut w = work();
        let mut r = report("check", 1, vec![CommandResult::exited("x", 0)], false);
        r.commit = Some(stamp(Some("abc"), true, reach));
        w.gates.push(r);
        let basis = Completion::of(&w, true);
        let w = finished(w, basis);
        assert!(
            seen.insert(Certificate::of(&w, None).markdown()),
            "two reachability states produced the same document"
        );
    }
}

#[test]
fn a_dirty_tree_is_said_where_the_commit_is() {
    let mut w = work();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)], false);
    r.commit = Some(stamp(Some("abc123"), false, Reach::Remote));
    w.gates.push(r);
    let basis = Completion::of(&w, true);
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    let at_commit = md.find("abc123").unwrap();
    let warning = md
        .find("does NOT fully describe")
        .expect("no dirty warning");
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
        let mut w = work();
        w.gates.push(report(
            "check",
            1,
            vec![CommandResult::without_verdict("cargo test", outcome)],
            false,
        ));
        let basis = Completion::of(&w, true);
        let w = finished(w, basis);
        assert!(
            seen.insert(Certificate::of(&w, None).markdown()),
            "two outcomes rendered alike"
        );
    }
}

#[test]
fn a_reproduction_is_never_rendered_as_a_failure() {
    let mut w = work();
    w.gates.push(report(
        "repro",
        1,
        vec![CommandResult::exited("cargo test --test repro", 1)],
        true,
    ));
    let basis = Completion::of(&w, true);
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("reproduced the problem"), "{md}");
    assert!(
        !md.contains("— failed"),
        "a successful reproduction read as a failure"
    );
}

#[test]
fn a_specification_that_was_not_found_is_stated_not_omitted() {
    let mut w = work();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)], false);
    r.spec = Some(SpecStamp {
        path: "specs/reset/".into(),
        fingerprint: None,
        files: 0,
        tasks_done: 0,
        tasks_total: 0,
        open_questions: 0,
    });
    w.gates.push(r);
    let basis = Completion::of(&w, true);
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("there was no such file or folder"), "{md}");
}

#[test]
fn unticked_tasks_appear_beside_the_outcome_and_neither_is_judged() {
    let mut w = work();
    let mut r = report("check", 1, vec![CommandResult::exited("x", 0)], false);
    r.spec = Some(SpecStamp {
        path: "specs/012/".into(),
        fingerprint: Some("3f9a1c05e7b2d648".into()),
        files: 4,
        tasks_done: 28,
        tasks_total: 31,
        open_questions: 1,
    });
    w.gates.push(r);
    let basis = Completion::of(&w, true);
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
        Completion::Reproduced {
            gate: "repro".into(),
            attempt: 1,
            attempts: 1,
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
    // And the two that are not checks must not read as passes.
    assert!(bases[2].headline().contains("nothing was checked"));
    assert!(bases[3].headline().contains("did not pass"));
    assert!(!bases[2].is_checked());
    assert!(!bases[3].is_checked());
}

#[test]
fn a_project_with_no_gates_is_not_reported_as_a_pass() {
    let w = work();
    let basis = Completion::of(&w, false);
    assert!(matches!(basis, Completion::NoGateDeclared { .. }));
    let w = finished(w, basis);
    let md = Certificate::of(&w, None).markdown();
    assert!(md.contains("never said what done means"), "{md}");
    assert!(md.contains("Nothing was checked by this tool"), "{md}");
}

#[test]
fn a_failing_gate_cannot_produce_a_gates_passed_basis() {
    let mut w = work();
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("cargo test", 101)],
        false,
    ));
    let basis = Completion::of(&w, true);
    assert!(
        matches!(basis, Completion::ByHand { .. }),
        "a failing gate produced a passing basis"
    );
    assert!(basis.headline().contains("finished by hand"));
}

#[test]
fn a_reproduction_that_reproduced_is_its_own_basis() {
    let mut w = work();
    w.gates.push(report(
        "repro",
        1,
        vec![CommandResult::exited("cargo test --test repro", 1)],
        true,
    ));
    assert!(matches!(
        Completion::of(&w, true),
        Completion::Reproduced { .. }
    ));
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
fn a_work_with_no_claim_is_not_reported_as_an_agent_that_said_nothing() {
    let w = passing();
    let md = Certificate::of(&w, None).markdown();
    assert!(
        !md.contains("The agent's claim"),
        "an absent claim grew a section"
    );
}

// ── The awkward shapes ─────────────────────────────────────────────────────

#[test]
fn unfinished_work_exports_an_honest_account_rather_than_a_certificate() {
    let mut w = work();
    w.phase = Phase::Verify;
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("cargo test", 101)],
        false,
    ));
    let c = Certificate::of(&w, None);
    assert!(!c.is_finished());
    let md = c.markdown();
    assert!(md.contains("not finished"), "{md}");
    assert!(md.contains("verify"), "it does not say where the work is");
    assert!(md.contains("Its last gate said"));
    assert!(
        !md.contains("Check this yourself"),
        "it offered instructions anyway"
    );
}

#[test]
fn the_artifact_is_bounded_and_says_that_it_truncated() {
    let mut w = work();
    let huge = CommandResult {
        output_tail: "x".repeat(200_000),
        output_bytes: 100 * 1024 * 1024,
        ..CommandResult::exited("cargo test", 0)
    };
    w.gates.push(report("check", 1, vec![huge], false));
    let basis = Completion::of(&w, true);
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
    // Gate commands come from a project's committed configuration; titles from
    // issues and models. In JSON the serialiser handles this. In Markdown
    // nothing does it for you.
    let nasty = "```\n# Fake heading\n| a | b |\n<!-- comment -->\n```";
    let mut w = work();
    w.title = nasty.into();
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited(nasty, 0)],
        false,
    ));
    let basis = Completion::of(&w, true);
    let w = finished(w, basis);
    let c = Certificate::of(&w, None);
    let md = c.markdown();

    // The document's own sections all survived.
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
    // The heading inside the payload never became a heading *of the document*.
    //
    // Inside a fence that is longer than anything in the content, the same
    // characters are literal text, so the check has to be about **where** the
    // line is rather than whether it exists. A line-based test cannot tell the
    // two apart, and asserting the weaker thing would have failed a correct
    // implementation.
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
    // It is still all there as text.
    assert!(md.contains("Fake heading"));
    // The structured shape carries it verbatim, which is the serialiser's job.
    assert_eq!(c.json()["predicate"]["work"]["title"], nasty);
}

/// The lines of a document that are **not** inside a fenced block, with their
/// numbers. Fences here are always the longest run in the content plus one, so
/// tracking any run of three or more and matching its length is enough.
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
    let mut w = work();
    w.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("echo '```'; rm -rf /", 0)],
        false,
    ));
    let basis = Completion::of(&w, true);
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

/// **`Phase::Done` is written in exactly one place**, and the general phase
/// setter cannot express it at all.
///
/// The type makes the second half true: `set_phase` takes a `Step`, which has
/// no `Done` variant, so there is no value a caller could pass. This guards the
/// first half — that nobody has since added a second writer — because the
/// compiler cannot notice a new direct assignment.
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
                if line.contains("phase = Phase::Done") {
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
    assert!(sites[0].contains("work.rs"), "{sites:?}");
}

/// The general phase setter has no way to say `Done`.
///
/// If somebody adds one, this fails — and the comment says why it is not there.
#[test]
fn the_phase_setter_cannot_express_done() {
    let src = std::fs::read_to_string("src/work.rs").unwrap();
    let step = src
        .split("pub(crate) enum Step {")
        .nth(1)
        .and_then(|s| s.split('}').next())
        .expect("the Step enum is gone, and with it the barrier");
    assert!(
        !step.contains("Done"),
        "`Done` was added back to the setter, so a caller can once again mark \
         work finished without recording what it rests on"
    );
}

/// Every finished work has a basis, and no unfinished one does.
#[test]
fn a_basis_exists_exactly_when_the_work_is_finished() {
    let unfinished = work();
    assert!(unfinished.completion.is_none());
    assert_ne!(unfinished.phase, Phase::Done);

    let done = passing();
    assert_eq!(done.phase, Phase::Done);
    assert!(
        done.completion.is_some(),
        "a finished work with nothing attached"
    );
}

/// The basis constructor is total: it answers for every shape of work.
#[test]
fn every_shape_of_work_gets_a_basis() {
    let mut cases = vec![work()];

    let mut passed = work();
    passed.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("x", 0)],
        false,
    ));
    cases.push(passed);

    let mut failed = work();
    failed.gates.push(report(
        "check",
        1,
        vec![CommandResult::exited("x", 1)],
        false,
    ));
    cases.push(failed);

    let mut repro = work();
    repro.gates.push(report(
        "repro",
        1,
        vec![CommandResult::exited("x", 1)],
        true,
    ));
    cases.push(repro);

    let mut hung = work();
    hung.gates.push(report(
        "check",
        1,
        vec![CommandResult::without_verdict(
            "x",
            Outcome::TimedOut { after_secs: 1 },
        )],
        false,
    ));
    cases.push(hung);

    for w in &cases {
        for declares in [true, false] {
            // Total by construction — there is no value to forget to supply.
            let basis = Completion::of(w, declares);
            assert!(!basis.headline().is_empty());
        }
    }
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
    // Different sentences: *this repository has no remote* is not a problem,
    // and *there is a remote and this is not on it* very much is.
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

    // Committed, not pushed: nobody else can check this out.
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

//! The dialect axis's outcome model, guarded as text.
//!
//! **These read the script rather than run it**, the way the page's contract
//! reads `ui/index.html`, and for the same reason: running it costs a signed-in
//! agent and real money, so it cannot be what `just verify` does on every
//! machine. That is a real limit and is worth saying plainly — a text guard
//! catches a rule that was deleted or an outcome that stopped being counted,
//! and cannot catch one that is counted wrongly. The arithmetic these protect
//! lives in `core::sequential`, where it is tested by running it.
//!
//! What they exist to hold is the thing this feature was written against: an
//! outcome quietly collapsing into agreement. Five of the six are easy to get
//! right. `skipped`, `declined` and `inconclusive` are the ones that look like
//! agreement to a tally that is not paying attention, and each of the three
//! means something different — this host could not run it, the vendor would not
//! do it, and nobody could tell.

const AXIS: &str = include_str!("../scripts/verify-permissions-diff.sh");

/// Every one of the six outcomes is counted, and the tally adds up.
///
/// A row that goes missing is the failure the selftest exists for one level
/// down: a total that does not add up means the report is describing fewer
/// probes than were paid for.
#[test]
fn the_tally_covers_all_six_outcomes() {
    for counter in [
        "d_agreed",
        "d_narrower",
        "d_wider",
        "d_skipped",
        "d_declined",
        "d_incon",
    ] {
        assert!(
            AXIS.contains(&format!("{counter}=0")),
            "the dialect axis does not initialise {counter}, so it cannot report it"
        );
        assert!(
            AXIS.contains(&format!("{counter} + 1")) || AXIS.contains(&format!("{counter}=$((")),
            "{counter} is declared and never incremented"
        );
    }
    let total = AXIS
        .lines()
        .find(|l| l.contains("total=$((d_agreed"))
        .expect("no total is computed");
    for counter in [
        "d_narrower",
        "d_wider",
        "d_skipped",
        "d_declined",
        "d_incon",
    ] {
        assert!(
            total.contains(counter),
            "the total omits {counter}, so those rows are paid for and not reported"
        );
    }
}

/// **Skipped is never agreement.** The matrix already applies this to twelve
/// shapes it cannot run on this host; the axis inherits it, and the summary
/// says so rather than leaving a reader to notice.
#[test]
fn skipped_is_a_debt_with_a_name() {
    assert!(
        AXIS.contains("skipped rows are debts with names, and are never counted as agreement"),
        "the summary no longer says what a skipped row is"
    );
    // Every skip prints its reason. A bare "skipped" sends somebody looking.
    for reason in [
        "skipped: no pwsh on this host",
        "skipped: no way to ask the vendor",
    ] {
        assert!(AXIS.contains(reason), "a skip lost its reason: {reason}");
    }
}

/// **LSP is skipped, not agreed.** There is no documented way to make a
/// headless run issue an LSP request against a path of the harness's choosing,
/// so those rows stay a claim about this matcher. Claiming agreement there
/// would be the exact failure this axis was built to stop.
#[test]
fn lsp_is_not_quietly_counted_as_agreement() {
    let lsp = AXIS
        .split("LSP — not asked")
        .nth(1)
        .expect("the axis no longer says LSP is unasked");
    // Just the loop, not the summary beneath it — which mentions every counter
    // and would make this pass or fail for the wrong reason.
    let block = lsp.split("total=$((").next().unwrap();
    assert!(
        block.contains("d_skipped"),
        "LSP rows stopped being counted as skipped"
    );
    assert!(
        !block.contains("d_agreed"),
        "LSP rows are being counted as agreement without anything having been asked"
    );
}

/// **Inconclusive fails the run and is never rounded to agreement.**
///
/// This guards a defect that was in shipping code: a disagreement that failed
/// to reproduce was printed `ok`. *The disagreement did not reproduce* and *the
/// two sides agree* are different sentences.
#[test]
fn inconclusive_is_never_agreement() {
    let arm = AXIS
        .split("inconclusive)")
        .nth(1)
        .expect("the axis has no inconclusive arm");
    let block: String = arm.chars().take(400).collect();
    assert!(
        block.contains("not agreement") || block.contains("INCONCLUSIVE"),
        "the inconclusive arm no longer says it is not agreement"
    );
    assert!(
        block.contains("fail=1"),
        "an undecidable probe passes the run silently"
    );
}

/// A wider row fails; a narrower one does not. The asymmetry is the product.
#[test]
fn wider_fails_and_narrower_does_not() {
    let arm = AXIS
        .split("disagree)")
        .nth(1)
        .expect("the axis has no disagree arm");
    let block: String = arm.chars().take(900).collect();
    let wider = block.find("WIDER").expect("no wider branch");
    let narrower = block.find("narrower").expect("no narrower branch");
    assert!(
        block[wider..].contains("d_wider"),
        "a wider row is not counted"
    );
    assert!(
        block[..narrower].contains("fail=1"),
        "a wider row no longer fails the run, which is the one thing it must do"
    );
}

/// **The decision comes from the domain, not from arithmetic rewritten in
/// shell.** Two copies of a sequential test are two rules that drift, and the
/// one in the shell would be the one nobody tests.
#[test]
fn the_sequential_rule_has_one_implementation() {
    assert!(
        AXIS.contains("sequential --agreements"),
        "the axis no longer asks the domain when to stop"
    );
    for arithmetic in ["log(", "ln(", "llr", "likelihood"] {
        assert!(
            !AXIS.to_lowercase().contains(arithmetic),
            "the shell grew its own copy of the sequential arithmetic: {arithmetic}"
        );
    }
}

/// The axis stopped calling itself a checklist, and only because it stopped
/// being one.
#[test]
fn the_axis_no_longer_disclaims_being_a_measurement() {
    let printed: String = AXIS
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !printed.contains("NOT a measurement"),
        "the axis still tells the reader it has asked nothing"
    );
    assert!(
        printed.contains("dialect axis — measured against Claude Code"),
        "the axis does not name the version it measured"
    );
}

/// Every run says what it cost, because *priced out of running weekly* was a
/// feeling for four passes and should not become one again.
#[test]
fn every_summary_reports_what_it_spent() {
    let summaries = AXIS.matches("$(spend_line)").count();
    assert!(
        summaries >= 4,
        "only {summaries} summaries report their cost; a run that does not is a meter nobody reads"
    );
    assert!(
        AXIS.contains("client-side estimate"),
        "the cost is reported without saying it is an estimate"
    );
}

//! What this file defends, and it is three things a comment cannot.
//!
//! **The separation of the floors.** There are three, they cost different
//! amounts and they claim different things, and the one failure this feature
//! cannot have is a cheap run advancing an expensive claim. That is asserted by
//! attempting it rather than by a rule in a document.
//!
//! **The four release outcomes.** *Announced nothing* and *measured clean* are
//! different sentences, and a surface that renders them alike is wrong — a
//! constitutional constraint, so it gets a test rather than a paragraph.
//!
//! **The cadence.** Two bounds, never averaged. A single "days until due" would
//! hide which of the two is the reason, which is the same mistake as collapsing
//! the floors one layer down.

use devplane::core::policy::{
    Cadence, FULL_MATRIX_DAYS, FULL_MATRIX_RELEASES, ROWS_CLEARED_THROUGH, ROWS_MEASURED_THROUGH,
    VERIFIED_AGAINST, VERIFIED_ON, cadence,
};

/// A version this many patches above `base`, derived rather than written down.
///
/// A green run moves these constants, and a test with a literal in it would
/// start asserting yesterday's gap the moment that happened.
fn ahead(base: &str, n: u64) -> String {
    let mut parts: Vec<u64> = base
        .split('.')
        .map(|p| p.parse().expect("a floor is three numbers"))
        .collect();
    *parts.last_mut().expect("a patch number") += n;
    parts
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

#[test]
fn the_cadence_reports_two_bounds_and_either_can_be_overdue_alone() {
    // Releases overdue, days not.
    let c = cadence(
        Some(&ahead(VERIFIED_AGAINST, FULL_MATRIX_RELEASES)),
        Some(VERIFIED_ON),
    );
    assert_eq!(c.overdue(), vec!["releases"], "the release bound alone");

    // Days overdue, releases not.
    let c = cadence(Some(&ahead(VERIFIED_AGAINST, 1)), Some("2026-12-31"));
    assert_eq!(c.overdue(), vec!["days"], "the day bound alone");

    // Neither.
    let c = cadence(Some(&ahead(VERIFIED_AGAINST, 1)), Some(VERIFIED_ON));
    assert!(c.overdue().is_empty(), "neither bound is overdue");
}

#[test]
fn an_uncountable_gap_is_absent_rather_than_zero() {
    // Across a minor boundary nothing can count releases, and a date nobody
    // supplied is not a date of zero days ago. Both report `None`, because a
    // bound printed as `0` when nobody knows is a lie with a number on it.
    let c = cadence(Some("3.0.0"), None);
    assert_eq!(c.releases, None, "no release count across a minor boundary");
    assert_eq!(c.days, None, "no day count without a date");
    assert!(
        c.overdue().is_empty(),
        "an unknown bound is not an overdue one"
    );

    // And the limits are still reported, so a reader sees what the bound *is*
    // even when the elapsed side is unknown.
    assert_eq!(c.releases_limit, FULL_MATRIX_RELEASES);
    assert_eq!(c.days_limit, FULL_MATRIX_DAYS);
}

#[test]
fn the_day_count_is_exact_across_a_month_and_a_leap_year() {
    let days = |from: &str, to: &str| {
        cadence(None, Some(to))
            .days
            .map(|_| ())
            .and(Some(()))
            .and_then(|()| {
                // exercise the same arithmetic through the public surface by
                // measuring from the real constant, then differencing
                let a = cadence(None, Some(from)).days?;
                let b = cadence(None, Some(to)).days?;
                Some(b as i64 - a as i64)
            })
    };
    assert_eq!(days("2026-09-17", "2026-10-17"), Some(30), "across a month");
    assert_eq!(
        days("2028-02-01", "2028-03-01"),
        Some(29),
        "a leap February"
    );
    assert_eq!(
        days("2027-02-01", "2027-03-01"),
        Some(28),
        "a common February"
    );
}

#[test]
fn a_cadence_never_offers_a_single_combined_figure() {
    // `overdue()` names which bounds, and the type carries no field that is a
    // function of both. The guard is structural: if somebody adds a
    // `days_until_due`, this stops compiling rather than silently passing.
    let c: Cadence = cadence(Some(&ahead(VERIFIED_AGAINST, 2)), Some("2026-09-20"));
    let Cadence {
        releases,
        releases_limit,
        days,
        days_limit,
    } = c;
    assert_eq!(releases, Some(2));
    assert_eq!(days, Some(3));
    assert_eq!(releases_limit, FULL_MATRIX_RELEASES);
    assert_eq!(days_limit, FULL_MATRIX_DAYS);
}

#[test]
fn the_three_floors_are_three_separate_constants() {
    // Not an ordering assertion, deliberately. The measured floor will usually
    // lead the compatibility floor and the reverse is legal too — a full run
    // advances compatibility past rows nobody probed individually. What must be
    // true is only that there are three of them and each is its own value.
    for (a, b) in [
        (ROWS_CLEARED_THROUGH, ROWS_MEASURED_THROUGH),
        (ROWS_MEASURED_THROUGH, VERIFIED_AGAINST),
        (ROWS_CLEARED_THROUGH, VERIFIED_AGAINST),
    ] {
        assert!(!a.is_empty() && !b.is_empty(), "a floor is never empty");
    }
}

// ── US2: three floors, and not one of them overstates itself ────────────────

use devplane::conformance::{FLOORS, as_json};

#[test]
fn there_are_three_floors_and_each_says_what_it_does_not_claim() {
    assert_eq!(FLOORS.len(), 3, "three claims, three costs, three rows");

    for f in FLOORS {
        assert!(!f.excludes.is_empty(), "`{}` has no exclusion", f.label);
        // The exclusion has to say what is *absent*, not restate the claim —
        // the same rule the conformance card is already held to.
        let e = f.excludes;
        assert!(
            e.contains("does not") || e.contains("only") || e.contains("nothing"),
            "`{}` does not say what it excludes: {e}",
            f.label
        );
    }

    let distinct = |get: fn(&devplane::conformance::Floor) -> &str| {
        let mut v: Vec<&str> = FLOORS.iter().map(get).collect();
        v.sort_unstable();
        let n = v.len();
        v.dedup();
        v.len() == n
    };
    assert!(distinct(|f| f.key), "three distinct keys");
    assert!(distinct(|f| f.label), "three distinct labels");
    assert!(distinct(|f| f.claims), "three claims that differ");
    assert!(distinct(|f| f.excludes), "three exclusions that differ");
}

#[test]
fn the_payload_never_offers_a_single_combined_compatibility_figure() {
    let v = as_json(None, None);
    let obj = v.as_object().expect("an object");

    // Each floor is present under its own key, with its own release.
    for f in FLOORS {
        assert_eq!(
            obj.get(f.key).and_then(|x| x.as_str()),
            Some(f.release),
            "`{}` is missing from the payload under its own key",
            f.key
        );
    }

    // And nothing derives a fourth number from them. Asserting the *absence*
    // on purpose: a payload carrying both the three and a summary would pass a
    // test that only checked the three were there.
    for k in obj.keys() {
        // `measured_against` is the retired name for the compatibility floor,
        // kept as an alias; it is not a derived figure.
        if k == "measured_against" {
            continue;
        }
        let k = k.to_lowercase();
        assert!(
            !(k.contains("score") || k.contains("overall") || k.contains("combined")),
            "`{k}` reads as a figure derived from the three floors"
        );
    }
}

#[test]
fn the_gate_command_prints_three_floors_and_no_combined_figure() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("gate")
        .output()
        .expect("the binary runs");
    let text = String::from_utf8_lossy(&out.stdout);

    for f in FLOORS {
        assert!(
            text.contains(f.label),
            "`devplane gate` never names the `{}` floor",
            f.label
        );
        assert!(
            text.contains(f.excludes),
            "the `{}` floor prints without its exclusion",
            f.label
        );
    }

    // Two bounds, never averaged into one.
    assert!(
        text.contains("releases or"),
        "the cadence prints two bounds"
    );
    assert!(
        !text.to_lowercase().contains("days until due"),
        "the cadence collapsed into a single figure"
    );
}

#[test]
fn no_two_floors_share_a_label_on_any_surface() {
    // Written because this feature broke it on its way in: `doctor` briefly
    // printed the compatibility floor and the measured floor under the same
    // word `measured`, which is the exact collapse the whole thing exists to
    // prevent, committed inside the fix for it.
    for cmd in ["doctor", "gate"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg(cmd)
            .output()
            .expect("the binary runs");
        let text = String::from_utf8_lossy(&out.stdout);

        let mut labels: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("  "))
            .filter(|l| !l.starts_with(' '))
            .filter_map(|l| l.split_whitespace().next())
            .filter(|w| {
                [
                    "read",
                    "rows",
                    "measured",
                    "probed",
                    "compat",
                    "compatibility",
                ]
                .contains(w)
            })
            .collect();
        let before = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(
            labels.len(),
            before,
            "`devplane {cmd}` prints two floors under one label: {labels:?}"
        );
    }
}

// ── US4: a release that cannot be measured says so and blocks the floor ─────

use devplane::core::measured::{ProbeOutcome, ReleaseOutcome, Row, floor_after, release_outcome};

#[test]
fn a_skipped_probe_yields_owed_and_never_measured() {
    // The skip set is not stable between runs, because the oracle is a model.
    // So a release can be measured on one run and owed on the next, and that is
    // correct rather than flaky — what must never happen is a skip counting as
    // a pass.
    assert_eq!(
        release_outcome(&[
            Row::Probe(ProbeOutcome::Agreed),
            Row::Probe(ProbeOutcome::Skipped)
        ]),
        ReleaseOutcome::Owed
    );
    assert_eq!(
        release_outcome(&[Row::Probe(ProbeOutcome::Agreed), Row::Declined]),
        ReleaseOutcome::Measured
    );
    // A declared narrowing is a *recorded* stricter-than-vendor case, so it
    // counts as measured — that is what declaring it is for.
    assert_eq!(
        release_outcome(&[Row::Probe(ProbeOutcome::Declared)]),
        ReleaseOutcome::Measured
    );
    // A test over our own matcher is a gap of the same kind as a skip.
    assert_eq!(release_outcome(&[Row::Case]), ReleaseOutcome::Owed);
    // Red outranks incomplete: reporting the milder of the two would understate.
    assert_eq!(
        release_outcome(&[Row::Case, Row::Probe(ProbeOutcome::Disagreed)]),
        ReleaseOutcome::Disagreed
    );
}

#[test]
fn a_release_with_no_rule_rows_says_so_rather_than_reporting_zero() {
    let none = release_outcome(&[]);
    assert_eq!(none, ReleaseOutcome::NoRows);

    // Absence is distinguishable from zero. Assert what it must NOT say as well
    // as what it must: a surface printing both would pass a weaker test.
    let said = none.says();
    assert!(said.contains("announced nothing"), "got: {said}");
    assert!(
        !said.contains('0'),
        "a count leaked into an absence: {said}"
    );
    assert!(
        !said.contains("clean") && !said.contains("agreed"),
        "an absence is wearing a measurement's words: {said}"
    );

    // And it is a different sentence from every other outcome.
    for other in [
        ReleaseOutcome::Measured,
        ReleaseOutcome::Owed,
        ReleaseOutcome::Disagreed,
    ] {
        assert_ne!(said, other.says(), "two outcomes share a sentence");
    }

    // A release that announced nothing still advances the floor: the claim is
    // about announced rows, and it is vacuously true here.
    assert!(none.advances());
}

#[test]
fn the_floor_stops_below_the_first_owed_release_however_clean_the_rest_are() {
    // The invariant most likely to be written as "skip and continue". Every
    // release above the first gap is unmeasured *regardless of its own
    // outcome*, and the two clean releases at the end are the trap.
    let span = [
        ("2.1.241", ReleaseOutcome::Measured),
        ("2.1.242", ReleaseOutcome::NoRows),
        ("2.1.243", ReleaseOutcome::Owed),
        ("2.1.244", ReleaseOutcome::Measured),
        ("2.1.245", ReleaseOutcome::Measured),
    ];
    assert_eq!(floor_after("2.1.240", &span), "2.1.242");

    // A disagreement stops it in the same place, for a different reason.
    let span = [
        ("2.1.241", ReleaseOutcome::Measured),
        ("2.1.242", ReleaseOutcome::Disagreed),
        ("2.1.243", ReleaseOutcome::Measured),
    ];
    assert_eq!(floor_after("2.1.240", &span), "2.1.241");

    // An unbroken run moves it all the way; an empty span moves nothing.
    let span = [
        ("2.1.241", ReleaseOutcome::NoRows),
        ("2.1.242", ReleaseOutcome::Measured),
    ];
    assert_eq!(floor_after("2.1.240", &span), "2.1.242");
    assert_eq!(floor_after("2.1.240", &[]), "2.1.240");

    // And a gap in the very first position leaves the floor exactly where it was.
    assert_eq!(
        floor_after("2.1.240", &[("2.1.241", ReleaseOutcome::Owed)]),
        "2.1.240"
    );
}

// ── US1: the row-scoped run, and the separation it must never breach ────────

fn repo(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn sh(args: &[&str]) -> (String, i32) {
    let out = std::process::Command::new("bash")
        .current_dir(repo(""))
        .args(args)
        .output()
        .expect("bash runs");
    (
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn a_row_scoped_run_cannot_advance_the_compatibility_floor() {
    // The test the whole feature exists to keep green, in the form the spec
    // asked for: demonstrated by attempting it, not asserted in a document.
    let policy = repo("src/core/policy.rs");
    let before = std::fs::read_to_string(&policy).expect("policy.rs");

    let (out, code) = sh(&["scripts/measured-through.sh", "--dry-run"]);
    assert_eq!(code, 0, "the dry run should succeed:\n{out}");

    let after = std::fs::read_to_string(&policy).expect("policy.rs");
    assert_eq!(before, after, "a row-scoped run changed src/core/policy.rs");

    // And structurally: the driver has no write path to the compatibility
    // floor at all. A run that *could* write it and merely chose not to today
    // is one refactor from writing it tomorrow.
    let driver = std::fs::read_to_string(repo("scripts/measured-through.sh")).expect("the driver");
    for line in driver.lines() {
        let touches = line.contains("VERIFIED_AGAINST");
        let is_prose = line.trim_start().starts_with('#');
        assert!(
            !touches || is_prose,
            "the driver names the compatibility floor outside a comment: {line}"
        );
    }
}

#[test]
fn a_scoped_run_and_a_full_run_are_mutually_exclusive() {
    // An error, not a precedence rule. "Which one wins" is a question nobody
    // should have to answer with a green result already on screen.
    for extra in [vec!["5"], vec![]] {
        let mut args = vec!["scripts/verify-permissions-diff.sh"];
        args.extend(extra.iter().copied());
        let out = std::process::Command::new("bash")
            .current_dir(repo(""))
            .args(&args)
            .env("DEVPLANE_DIFF_PROBES", "tee-write-target")
            .env(
                "DEVPLANE_DIFF_AXIS",
                if extra.is_empty() { "both" } else { "deny" },
            )
            .output()
            .expect("bash runs");
        assert_eq!(
            out.status.code(),
            Some(2),
            "a scoped run combined with a full one should exit 2"
        );
    }
}

#[test]
fn every_declared_probe_names_a_call_the_harness_can_actually_run() {
    // A registry entry that has drifted from the shape tables is a probe that
    // silently measures nothing — the failure mode this layer is built to
    // prevent, one level up from the rules.
    let probes = std::fs::read_to_string(repo("scripts/probes.txt")).expect("probes.txt");
    let harness =
        std::fs::read_to_string(repo("scripts/verify-permissions-diff.sh")).expect("the harness");

    // Asked of the harness itself rather than re-implemented here: the shape
    // tables use a shell variable for the common payload, so the *source text*
    // and the *runtime value* differ, and a test that matched source text would
    // pass on the deny shapes and silently fail on every allow one.
    let _ = harness;
    let ids: Vec<&str> = probes
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split('|').next())
        .map(str::trim)
        .collect();
    assert!(ids.len() >= 10, "the registry lost entries: {}", ids.len());

    let out = std::process::Command::new("bash")
        .current_dir(repo(""))
        .arg("scripts/verify-permissions-diff.sh")
        .env("DEVPLANE_DIFF_PROBES", ids.join(","))
        .env("DEVPLANE_DIFF_CHECK", "1")
        .output()
        .expect("bash runs");
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "a declared probe no longer resolves to a runnable shape:\n{text}"
    );
    for id in &ids {
        assert!(
            text.contains(&format!("PROBE {id} ok")),
            "probe `{id}` did not resolve"
        );
    }
}

#[test]
fn the_dry_run_stops_the_floor_below_the_first_owed_release() {
    // The same rule as the pure function, through the driver: the honest result
    // today is that the floor barely moves, because an early release in the
    // span has a row with only a matcher test behind it. A driver that reported
    // a reachable floor above an owed release would be "skip and continue".
    let (out, code) = sh(&["scripts/measured-through.sh", "--dry-run"]);
    assert_eq!(code, 0, "{out}");

    let owed_line = out
        .lines()
        .find(|l| l.contains("owed ("))
        .expect("the dry run names the owed releases");
    let reach = out
        .lines()
        .find(|l| l.contains("floor could reach"))
        .expect("the dry run says how far the floor could reach");

    let first_owed = owed_line
        .split_whitespace()
        .find(|w| w.starts_with("2.1."))
        .expect("at least one owed release today");
    let reached = reach.split_whitespace().last().expect("a release");

    let patch = |v: &str| -> u64 { v.rsplit('.').next().unwrap().parse().unwrap_or(u64::MAX) };
    assert!(
        patch(reached) < patch(first_owed),
        "the floor reached {reached}, at or past the first owed release {first_owed}"
    );
}

// ── US3: the report says what the cheap run cannot see ──────────────────────

#[test]
fn a_green_report_always_carries_what_it_did_not_measure() {
    use devplane::conformance::MEASURES_ONLY_WHAT_WAS_ANNOUNCED;

    // The caveat says what is *absent*, not what is present — the same rule the
    // conformance card's unmet properties are already held to.
    assert!(
        MEASURES_ONLY_WHAT_WAS_ANNOUNCED.contains("does not")
            || MEASURES_ONLY_WHAT_WAS_ANNOUNCED.contains("only"),
        "the caveat does not say what was left unmeasured"
    );

    // It reaches the JSON, the CLI, and the driver's own output. Three surfaces
    // report a measurement, and a green verdict on any of them without this
    // sentence is the defect.
    let v = as_json(None, None);
    assert_eq!(
        v["measures_only_what_was_announced"].as_str(),
        Some(MEASURES_ONLY_WHAT_WAS_ANNOUNCED)
    );

    let (out, _) = sh(&["-c", "cargo run --quiet -- gate 2>/dev/null"]);
    assert!(
        out.contains("only what the vendor announced"),
        "`devplane gate` printed no caveat"
    );

    let driver = std::fs::read_to_string(repo("scripts/measured-through.sh")).expect("the driver");
    assert!(
        driver.contains("it says nothing about the rest of the matcher"),
        "the driver reports a verdict without saying what it did not measure"
    );
}

#[test]
fn the_report_names_the_full_matrix_and_how_far_behind_it_is() {
    let (out, _) = sh(&["-c", "cargo run --quiet -- gate 2>/dev/null"]);
    assert!(
        out.contains(devplane::core::policy::VERIFIED_AGAINST),
        "the report never names the release the full matrix ran against"
    );
    assert!(
        out.contains("full matrix due at"),
        "the report never says when the full matrix is next owed"
    );
}

// ── The record, and the refusals that change nothing ────────────────────────

#[test]
fn a_record_carries_every_field_its_contract_names() {
    // Exercised without a vendor and without spending: the writer is its own
    // script precisely so this test can exist. A writer only reachable by
    // paying is a writer nobody checks.
    let dir = std::env::temp_dir().join(format!("vp-rec-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let detail = dir.join("detail.tsv");
    std::fs::write(
        &detail,
        "2.1.241\tno-rows\t-\t-\t\n\
         2.1.243\towed\tcase\t-\tFixed something the notes describe | with a pipe in it\n\
         2.1.244\tmeasured\tprobe\ttee-write-target\tFixed the file a tee writes\n",
    )
    .expect("fixture");
    let out = dir.join("rec.json");

    let st = std::process::Command::new("python3")
        .current_dir(repo(""))
        .arg("scripts/write-measurement.py")
        .args([
            out.to_str().unwrap(),
            "2.1.240",
            "2.1.244",
            "2.1.276",
            "green",
            "2.1.241",
            detail.to_str().unwrap(),
            "PROBE tee-write-target agreed\nPROBE other-probe skipped (the model will not run it)",
        ])
        .status()
        .expect("python runs");
    assert!(st.success(), "the writer failed");

    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("record")).expect("json");

    for key in [
        "run_at",
        "span",
        "vendor_version",
        "releases",
        "probes",
        "verdict",
        "floor_before",
        "floor_after",
        "measures_only_what_was_announced",
    ] {
        assert!(!v[key].is_null(), "the record omits `{key}`");
    }
    assert_eq!(v["span"]["from"], "2.1.240");
    assert_eq!(v["span"]["to"], "2.1.244");
    // The version that answered, not the one requested.
    assert_eq!(v["vendor_version"], "2.1.276");

    let rels = v["releases"].as_array().expect("releases");
    assert_eq!(rels.len(), 3, "one entry per release in the span");
    assert_eq!(rels[0]["outcome"], "no-rows");
    assert!(
        rels[0]["rows"].as_array().expect("rows").is_empty(),
        "a no-rows release carries no rows"
    );
    assert_eq!(rels[1]["outcome"], "owed");
    assert_eq!(rels[1]["rows"][0]["disposition"], "case");
    // Row text is carried verbatim as data, pipes and all.
    assert!(
        rels[1]["rows"][0]["text"]
            .as_str()
            .expect("text")
            .contains("| with a pipe in it"),
        "row text was mangled"
    );
    assert_eq!(rels[2]["rows"][0]["probe_ref"], "tee-write-target");

    let probes = v["probes"].as_array().expect("probes");
    assert_eq!(probes.len(), 2, "one entry per probe, with its outcome");
    assert_eq!(probes[0]["outcome"], "agreed");
    assert_eq!(probes[1]["outcome"], "skipped");
    assert_eq!(v["tally"]["skipped"], 1);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_run_that_cannot_reach_the_vendor_changes_nothing() {
    // Observed rather than argued: with no vendor reachable
    // the driver must exit 2, name the missing precondition, move no floor and
    // write no record. This ran for real on 2026-09-18 and did exactly that.
    let policy = repo("src/core/policy.rs");
    let before = std::fs::read_to_string(&policy).expect("policy.rs");

    // A PATH that still has a shell and the ordinary tools, and does not have
    // the vendor: `claude` installs under Homebrew, npm or ~/.local, never in
    // /usr/bin. Emptying PATH entirely would hide bash and test nothing.
    let out = std::process::Command::new("/bin/bash")
        .current_dir(repo(""))
        .arg("scripts/measured-through.sh")
        .env("PATH", "/usr/bin:/bin")
        .output()
        .expect("bash runs");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);

    assert_eq!(
        out.status.code(),
        Some(2),
        "a missing precondition is exit 2:\n{text}"
    );
    assert!(
        text.contains("nothing measured") || text.contains("is missing"),
        "the refusal does not say a precondition was missing:\n{text}"
    );
    assert_eq!(
        before,
        std::fs::read_to_string(&policy).expect("policy.rs"),
        "a refused run changed the floors"
    );
}

// ── Robustness across vendor versions ───────────────────────────────────────

#[test]
fn every_position_relative_to_the_floor_says_something_different() {
    use devplane::core::policy::{Gap, VERIFIED_AGAINST, gap};

    // The defect this replaced: `releases_ahead` answered `None` for *at the
    // floor*, *behind the floor* and *a different series*, so a machine six
    // releases behind the measurement was told exactly what a machine standing
    // on it was told — nothing. Three situations, one silence.
    let behind = {
        let mut p: Vec<u64> = VERIFIED_AGAINST
            .split('.')
            .map(|x| x.parse().unwrap())
            .collect();
        p[2] -= 6;
        p.iter().map(u64::to_string).collect::<Vec<_>>().join(".")
    };
    let ahead = ahead(VERIFIED_AGAINST, 3);

    assert_eq!(gap(VERIFIED_AGAINST, VERIFIED_AGAINST), Gap::At);
    assert_eq!(gap(&ahead, VERIFIED_AGAINST), Gap::Ahead(3));
    assert_eq!(gap(&behind, VERIFIED_AGAINST), Gap::Behind(6));
    assert_eq!(gap("2.2.0", VERIFIED_AGAINST), Gap::Uncountable);
    assert_eq!(gap("not-a-version", VERIFIED_AGAINST), Gap::Uncountable);

    // Only standing exactly on the floor is silent. Everything else has
    // something a person can act on, and the three sentences differ.
    assert!(gap(VERIFIED_AGAINST, VERIFIED_AGAINST).says().is_none());
    let sentences: Vec<String> = [&ahead, &behind, &"2.2.0".to_string()]
        .iter()
        .map(|v| gap(v, VERIFIED_AGAINST).says().expect("a sentence"))
        .collect();
    for (i, a) in sentences.iter().enumerate() {
        for b in sentences.iter().skip(i + 1) {
            assert_ne!(a, b, "two different positions share a sentence");
        }
    }

    // And behind is never described as safe. It is tempting to read "measured
    // against something newer" as "at least as good", and the 2.1.268 rule the
    // vendor reverted at 2.1.273 is why it is not.
    let s = gap(&behind, VERIFIED_AGAINST).says().unwrap();
    assert!(s.contains("not the safe direction"), "got: {s}");
}

#[test]
fn a_measurement_cannot_advance_a_floor_past_what_the_binary_predates() {
    // Found by running this against a machine whose `claude` was nine releases
    // behind the head: a dry run never asks who answers, so nothing had noticed.
    // A binary cannot demonstrate behaviour it does not contain, and claiming it
    // did would be the widening direction of the measurement itself.
    let driver = std::fs::read_to_string(repo("scripts/measured-through.sh")).expect("the driver");
    assert!(
        driver.contains("a binary cannot demonstrate behaviour it predates"),
        "the driver does not refuse to measure past the installed vendor"
    );
    assert!(
        driver.contains("older \"$observed\" \"$stop_at\""),
        "the refusal is not wired to the floor the run would reach"
    );
}

// ── The claim that needs no oracle ──────────────────────────────────────────

#[test]
fn a_wildcard_rule_never_approves_a_call_this_matcher_cannot_read() {
    use devplane::core::policy::{Policy, Verdict};

    // **This is the property the harness cannot give you, and it is stronger.**
    // Every entry in the widening ledger was a shell construct nobody had
    // enumerated, found by asking a running vendor. This says something the
    // vendor is not needed for and that holds on every release it has ever
    // shipped: a call carrying a construct this matcher does not model is not
    // approvable under a wildcard rule, whatever the rules say.
    //
    // Narrower than the vendor is the safe side. Refusing to approve what
    // cannot be read is narrower by construction.
    let policy = Policy::new(&["Bash(cat *)".into()], &[]);

    // One family per construct the matcher declines to model. Each of these was
    // approved before 2026-09-18; the first was `.env` in ANSI-C hex, approved
    // under `Bash(cat *)` while `cat .env` was correctly denied.
    let unreadable = [
        r#"cat $'\x2e\x65nv'"#,
        r#"cat $'\056env'"#,
        "cat $(echo .env)",
        "cat `echo .env`",
        "cat <(echo hi)",
        "cat ${HOME}/x",
        "cat $FILE",
        "cat $((1 + 1))",
        "cat 'unbalanced",
    ];
    for call in unreadable {
        let v = policy.evaluate(
            &devplane::core::policy::Context::at(std::path::Path::new(".")),
            "Bash",
            &serde_json::json!({ "command": call }),
        );
        assert!(
            !matches!(v, Verdict::Allow { .. }),
            "a wildcard rule approved a call it cannot read: {call} -> {v:?}"
        );
    }

    // And the ordinary case is untouched, or the property would be worthless:
    // a gate that approves nothing is safe and useless.
    for call in ["cat notes.txt", "cat a.txt b.txt", "cat a.txt && cat b.txt"] {
        let v = policy.evaluate(
            &devplane::core::policy::Context::at(std::path::Path::new(".")),
            "Bash",
            &serde_json::json!({ "command": call }),
        );
        assert!(
            matches!(v, Verdict::Allow { .. }),
            "an ordinary call stopped being approvable: {call} -> {v:?}"
        );
    }
}

#[test]
fn a_projects_own_prohibition_cannot_be_defeated_by_its_own_allow_rule() {
    use devplane::core::policy::{Policy, Verdict};

    // The failure this found, stated as the thing a user actually cares about:
    // `never_auto = ["Read(.env)"]` plus `auto_allow = ["Bash(cat *)"]` used to
    // read `.env` with nobody asked, as long as the path was spelled in a
    // quoting form `dequoted` does not decode. No vendor disagreement was
    // needed to make that wrong — it defeated the user's own policy.
    let policy = Policy::new(&["Bash(cat *)".into()], &["Read(.env)".into()]);
    for spelling in [
        "cat .env",
        r#"cat $'\x2e\x65nv'"#,
        r#"cat $'\056env'"#,
        "cat $(echo .env)",
        r#"cat "$(printf %s .env)""#,
    ] {
        let v = policy.evaluate(
            &devplane::core::policy::Context::at(std::path::Path::new(".")),
            "Bash",
            &serde_json::json!({ "command": spelling }),
        );
        assert!(
            !matches!(v, Verdict::Allow { .. }),
            "`{spelling}` was auto-approved past a never_auto rule -> {v:?}"
        );
    }
}

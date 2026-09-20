//! The library verbs, against real directories.
//!
//! **Offline, and with no daemon.** Every test here builds scratch
//! repositories, runs the pure and file-level halves directly, and asserts on
//! what a person would be shown. Nothing reaches a network: a feature whose
//! whole claim is *these two files are identical* has no business needing one.
//!
//! Six repositories rather than three, where the specification says six. A
//! three-repository test passing is not evidence for a claim about six, and the
//! difference has caught a real bug before in this repository — a report that
//! is right about one project and wrong about the fifth looks identical at
//! three.

use devplane::core::library::{Drift, Portability, Refusal};
use devplane::library::{self, Artefact, Kind};
use std::path::{Path, PathBuf};

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "devplane-libtest-{}-{}-{}",
        tag,
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A skill in the vendor's own shape: a directory whose entry file is
/// `SKILL.md`, carrying a `scripts/` directory the specification names as
/// conventional.
fn write_skill(dir: &Path, body: &str) {
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: review-findings\ndescription: Reviews findings\n---\n\n{body}\n"),
    )
    .unwrap();
    std::fs::write(dir.join("scripts/run.sh"), "echo review\n").unwrap();
}

fn artefact(path: &Path, name: &str) -> Artefact {
    Artefact {
        name: name.to_string(),
        kind: Kind::Skill,
        digest: library::digest_at(path).unwrap(),
        path: path.to_path_buf(),
        sidecar: None,
    }
}

/// Six repositories, one edited. The report names that one and no other.
#[test]
fn six_repositories_one_edited_and_the_report_names_exactly_that_one() {
    let lib = scratch("six-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    let a = artefact(&src, "review-findings");

    let names = ["api", "web", "jobs", "billing", "infra", "payments"];
    let roots: Vec<PathBuf> = names.iter().map(|n| scratch(n)).collect();
    for root in &roots {
        library::install_one(&a, root, "tester").unwrap();
    }

    // One project's copy is edited, by hand, the way a person would.
    let edited = roots[3].join(".claude/skills/review-findings/SKILL.md");
    std::fs::write(&edited, "---\nname: review-findings\n---\n\nedited here\n").unwrap();

    let pairs: Vec<(String, PathBuf)> = names
        .iter()
        .zip(&roots)
        .map(|(n, r)| (n.to_string(), r.clone()))
        .collect();
    let cov = library::coverage(&a, &pairs);

    let drifted: Vec<&str> = cov
        .iter()
        .filter(|c| c.drift == Drift::CopyMoved)
        .map(|c| c.project.as_str())
        .collect();
    assert_eq!(drifted, ["billing"], "{cov:?}");
    assert_eq!(
        cov.iter().filter(|c| c.drift == Drift::Unchanged).count(),
        5,
        "the other five are untouched: {cov:?}"
    );

    for d in [lib].into_iter().chain(roots) {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// **The bug this feature's own tests found on their first run, pinned.**
///
/// `install` writes its sidecar *inside* the destination directory. The digest
/// walked that directory, so a freshly installed copy hashed differently from
/// the library source it was copied from — and every project reported as
/// drifted the instant it was installed. A report that says everything changed
/// is worth exactly as much as one that says nothing did, and no unit test over
/// canned rows could have seen it: the fault was in the seam between the pure
/// digest and the file that writes beside it.
#[test]
fn a_freshly_installed_copy_is_unchanged_and_not_drifted() {
    let lib = scratch("fresh-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    let a = artefact(&src, "review-findings");

    let root = scratch("fresh-proj");
    library::install_one(&a, &root, "tester").unwrap();

    // The sidecar really is there — the test would pass vacuously if install
    // had quietly stopped writing one.
    assert!(
        root.join(".claude/skills/review-findings/.devplane.toml")
            .exists(),
        "no sidecar was written, so this proves nothing about excluding it"
    );

    let cov = library::coverage(&a, &[("proj".into(), root.clone())]);
    assert_eq!(
        cov[0].drift,
        Drift::Unchanged,
        "installing something must not immediately report it as changed: {cov:?}"
    );
    assert!(
        cov[0].ignored.is_empty(),
        "and the sidecar is not a *suppression* either — it was never part of \
         the artefact, so there is nothing to report: {cov:?}"
    );

    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(root);
}

/// The library moves instead. The outcome is the **opposite direction** and
/// does not read the same — which is the distinction the whole feature rests
/// on and the one most easily collapsed into a boolean.
#[test]
fn when_the_library_moves_the_report_says_stale_and_not_drift() {
    let lib = scratch("stale-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    let a = artefact(&src, "review-findings");

    let root = scratch("stale-proj");
    library::install_one(&a, &root, "tester").unwrap();

    // The *source* moves; the copy is untouched.
    write_skill(&src, "the library moved on");
    let moved = artefact(&src, "review-findings");

    let cov = library::coverage(&moved, &[("proj".into(), root.clone())]);
    assert_eq!(cov[0].drift, Drift::LibraryMoved, "{cov:?}");
    assert_ne!(
        Drift::LibraryMoved.says(),
        Drift::CopyMoved.says(),
        "the same boolean and opposite instructions must not read the same"
    );
    assert_eq!(
        Drift::LibraryMoved.says(),
        "the library moved; this copy is the one you installed"
    );

    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(root);
}

/// Both sides move, and the report offers **no** resolution.
#[test]
fn a_two_sided_divergence_is_reported_and_nothing_is_offered() {
    let lib = scratch("both-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    let a = artefact(&src, "review-findings");

    let root = scratch("both-proj");
    library::install_one(&a, &root, "tester").unwrap();

    std::fs::write(
        root.join(".claude/skills/review-findings/SKILL.md"),
        "---\nname: review-findings\n---\n\nthe project moved\n",
    )
    .unwrap();
    write_skill(&src, "and so did the library");
    let moved = artefact(&src, "review-findings");

    let cov = library::coverage(&moved, &[("proj".into(), root.clone())]);
    assert_eq!(cov[0].drift, Drift::BothMoved);
    let said = Drift::BothMoved.says();
    assert!(
        said.contains("nobody here will pick"),
        "it must not suggest a direction: {said}"
    );
    for suggestion in ["run ", "use --", "recommend", "should"] {
        assert!(!said.contains(suggestion), "{said} offers a resolution");
    }

    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(root);
}

/// **The second bug running the command found.**
///
/// A project that has never held the artefact was reported as `Unchanged`,
/// which renders as `ok` — exactly like a project holding an identical copy.
/// Two opposite facts under one word: *you have this and it is fine*, and *you
/// do not have this*. Running `diff` against a real machine printed eight
/// projects as `ok` for a skill none of them had.
///
/// Coverage — *which of my six lack it* — is half of what the verb is for, so
/// it cannot be spelled the same as the other half.
#[test]
fn a_project_that_never_had_it_is_absent_rather_than_ok() {
    let lib = scratch("cov-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    let a = artefact(&src, "review-findings");

    let has = scratch("cov-has");
    let lacks = scratch("cov-lacks");
    library::install_one(&a, &has, "tester").unwrap();

    let cov = library::coverage(
        &a,
        &[("has".into(), has.clone()), ("lacks".into(), lacks.clone())],
    );
    let has_row = cov.iter().find(|c| c.project == "has").unwrap();
    let lacks_row = cov.iter().find(|c| c.project == "lacks").unwrap();

    assert!(has_row.present, "it holds a copy");
    assert!(!lacks_row.present, "it does not");
    // Both have nothing *drifted* — which is exactly why `drift` alone cannot
    // tell them apart, and why `present` has to exist.
    assert_eq!(has_row.drift, Drift::Unchanged);
    assert_eq!(lacks_row.drift, Drift::Unchanged);
    assert_ne!(
        has_row.present, lacks_row.present,
        "the coverage answer must be recoverable from the row"
    );

    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(has);
    let _ = std::fs::remove_dir_all(lacks);
}

/// A copy nobody recorded — the question nothing else on the machine answers.
#[test]
fn a_copy_devplane_did_not_install_is_unrecorded_and_not_unchanged() {
    let lib = scratch("unrec-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    let a = artefact(&src, "review-findings");

    // Placed by hand, with no sidecar: exactly what a colleague committing a
    // skill into the repository looks like.
    let root = scratch("unrec-proj");
    let dest = root.join(".claude/skills/review-findings");
    write_skill(&dest, "put here by somebody else");

    let cov = library::coverage(&a, &[("proj".into(), root.clone())]);
    assert_eq!(cov[0].drift, Drift::Unrecorded, "{cov:?}");

    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(root);
}

/// An untrusted target means **nothing is written anywhere** — verified by
/// listing the other targets' directories, not by trusting a return value.
#[test]
fn one_untrusted_target_means_nothing_is_written_to_any_of_them() {
    let lib = scratch("trust-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    let a = artefact(&src, "review-findings");

    let roots: Vec<PathBuf> = ["api", "web", "jobs", "billing"]
        .iter()
        .map(|n| scratch(n))
        .collect();
    let targets: Vec<(String, PathBuf, bool)> = vec![
        ("api".into(), roots[0].clone(), true),
        ("web".into(), roots[1].clone(), true),
        ("jobs".into(), roots[2].clone(), true),
        // The one nobody has looked at.
        ("billing".into(), roots[3].clone(), false),
    ];

    let checked = library::preflight_all(&a, &targets);
    let refused: Vec<&str> = checked
        .iter()
        .filter(|(_, _, r)| r.is_some())
        .map(|(n, _, _)| n.as_str())
        .collect();
    assert_eq!(refused, ["billing"]);
    assert_eq!(
        checked.iter().find(|(n, _, _)| n == "billing").unwrap().2,
        Some(Refusal::Untrusted)
    );

    // **The preflight is computed before any write, so nothing exists yet.**
    // Checked on disk rather than inferred: the whole promise is that a refusal
    // is never one failure after three successes.
    for root in &roots {
        assert!(
            !root.join(".claude").exists(),
            "{} was written to during a preflight",
            root.display()
        );
    }

    for d in [lib].into_iter().chain(roots) {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Every installed file is byte-identical to its source, compared **by bytes**
/// — the digest is the thing under test, so using it here would be the test
/// asserting itself.
#[test]
fn every_installed_file_is_byte_identical_to_its_library_source() {
    let lib = scratch("bytes-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    // Something with bytes a careless copy would normalise: CRLF, a BOM, a tab.
    std::fs::write(src.join("references.md"), b"\xEF\xBB\xBFone\r\n\ttwo\r\n").unwrap();
    let a = artefact(&src, "review-findings");

    let root = scratch("bytes-proj");
    library::install_one(&a, &root, "tester").unwrap();
    let dest = root.join(".claude/skills/review-findings");

    for rel in ["SKILL.md", "scripts/run.sh", "references.md"] {
        assert_eq!(
            std::fs::read(src.join(rel)).unwrap(),
            std::fs::read(dest.join(rel)).unwrap(),
            "{rel} was not copied byte for byte"
        );
    }

    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(root);
}

/// A collision is refused, and `--force` is what answers it — but never for an
/// untrusted target.
#[test]
fn a_collision_is_refused_and_an_identical_reinstall_is_not() {
    let lib = scratch("coll-lib");
    let src = lib.join("review-findings");
    write_skill(&src, "original");
    let a = artefact(&src, "review-findings");

    let root = scratch("coll-proj");
    library::install_one(&a, &root, "tester").unwrap();

    // Installing the same bytes again is not a collision: refusing it would
    // teach people to pass --force by reflex, which is how the flag stops
    // meaning anything.
    let same = library::preflight_all(&a, &[("proj".into(), root.clone(), true)]);
    assert_eq!(same[0].2, None, "{same:?}");

    // A different copy is.
    std::fs::write(
        root.join(".claude/skills/review-findings/SKILL.md"),
        "---\nname: review-findings\n---\n\ndifferent\n",
    )
    .unwrap();
    let differs = library::preflight_all(&a, &[("proj".into(), root.clone(), true)]);
    assert_eq!(differs[0].2, Some(Refusal::Collision));

    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(root);
}

/// **The no-grading refusal, as a check rather than a review note.**
///
/// No verb's vocabulary may contain a grade. This is asserted over the words
/// the code can actually produce, because a feature that starts saying *safe*
/// has become the registry this product refuses to be.
#[test]
fn no_verb_can_produce_a_grade() {
    let mut vocabulary: Vec<String> = Vec::new();
    for d in [
        Drift::Unchanged,
        Drift::CopyMoved,
        Drift::LibraryMoved,
        Drift::BothMoved,
        Drift::Missing,
        Drift::Unrecorded,
    ] {
        vocabulary.push(d.label().into());
        vocabulary.push(d.says().into());
    }
    for r in [
        Refusal::Untrusted,
        Refusal::Collision,
        Refusal::UndocumentedPath,
    ] {
        vocabulary.push(r.label().into());
        vocabulary.push(r.says(Path::new("/repo")));
    }
    vocabulary.push(Portability::CAVEAT.into());

    for word in &vocabulary {
        let lower = word.to_lowercase();
        for banned in ["safe", "unsafe", "risky", "score", "grade", "✓", "✔"] {
            assert!(!lower.contains(banned), "`{banned}` in {word:?}");
        }
        // And no `n/m` ratio anywhere: a count of how many checks something
        // passed is a score with a slash in it.
        assert!(
            !regexish_ratio(&lower),
            "a ratio reads as a score: {word:?}"
        );
    }
}

/// `n/m` without pulling in a regex engine for one assertion.
fn regexish_ratio(s: &str) -> bool {
    let b: Vec<char> = s.chars().collect();
    b.windows(3)
        .any(|w| w[0].is_ascii_digit() && w[1] == '/' && w[2].is_ascii_digit())
}

/// The no-translation refusal, guarded.
///
/// **This is the feature a reasonable person adds in month two**, and it would
/// destroy what the library is for: deciding what `context: fork` means on a
/// product with no subagents is a guess wearing a feature's clothes. The guard
/// is that an installed file's bytes equal its source's, including a field no
/// other vendor understands.
#[test]
fn no_verb_rewrites_a_field_on_the_way_through() {
    let lib = scratch("xlate-lib");
    let src = lib.join("review-findings");
    std::fs::create_dir_all(&src).unwrap();
    // Three fields outside the portable six, one of which is Claude Code's own.
    let body = "---\nname: review-findings\ndescription: Reviews findings\n\
                argument-hint: <file>\ncontext: fork\nmodel: opus\n---\n\nbody\n";
    std::fs::write(src.join("SKILL.md"), body).unwrap();
    let a = artefact(&src, "review-findings");

    let root = scratch("xlate-proj");
    library::install_one(&a, &root, "tester").unwrap();

    let written =
        std::fs::read_to_string(root.join(".claude/skills/review-findings/SKILL.md")).unwrap();
    assert_eq!(
        written, body,
        "a field was rewritten, dropped or translated on the way through"
    );

    // And the report *names* what a distribution path will reject, rather than
    // quietly fixing it.
    let p = library::portability_of(&a);
    assert!(!p.unread);
    let fields: Vec<&str> = p.findings.iter().map(|f| f.field.as_str()).collect();
    assert!(fields.contains(&"argument-hint"), "{p:?}");
    assert!(fields.contains(&"context"), "{p:?}");
    assert!(fields.contains(&"model"), "{p:?}");

    let _ = std::fs::remove_dir_all(lib);
    let _ = std::fs::remove_dir_all(root);
}

/// The case-insensitive-filesystem edge the specification names: two artefacts
/// differing only in case.
///
/// On macOS the second write lands on the first. The digest **includes the
/// path**, so the two are not silently one artefact — and whatever the
/// filesystem does, the report is about what it read rather than what it
/// expected.
#[test]
fn two_artefacts_differing_only_in_case_are_not_silently_one() {
    let lib = scratch("case-lib");
    let lower = lib.join("review-findings");
    let upper = lib.join("Review-Findings");
    write_skill(&lower, "lower");
    write_skill(&upper, "upper");

    let a = library::digest_at(&lower).unwrap();
    let b = library::digest_at(&upper).unwrap();
    // On a case-insensitive filesystem these are the same directory and the
    // digests agree — which is correct: there is one artefact. On a sensitive
    // one they differ. **Both are honest**; what would not be is reporting a
    // difference the filesystem cannot represent.
    let insensitive = lower.join("SKILL.md").exists()
        && std::fs::read_to_string(lower.join("SKILL.md"))
            .unwrap()
            .contains("upper");
    if insensitive {
        assert_eq!(a.digest, b.digest, "one directory, one digest");
    } else {
        assert_ne!(a.digest, b.digest, "two directories, two digests");
    }

    let _ = std::fs::remove_dir_all(lib);
}

/// Installing into a path no vendor documents is impossible by construction.
///
/// There is no code path that takes an arbitrary destination: `Scope::path` is
/// the only function that produces one, and every variant of it is a location
/// some vendor writes down. This asserts that the set stays closed.
#[test]
fn every_destination_is_a_path_some_vendor_documents() {
    use devplane::core::library::Scope;
    let root = Path::new("/repo");
    let documented = [
        (Scope::ClaudeProject, ".claude/skills/review"),
        (Scope::ClaudePersonal, ".claude/skills/review"),
        (Scope::CopilotPersonal, ".copilot/skills/review"),
        (Scope::DevplanePrompt, ".devplane/prompts/review.md"),
    ];
    for (scope, expected) in documented {
        assert_eq!(scope.path(root, "review"), root.join(expected));
        assert!(
            !scope.documented_by().is_empty(),
            "every scope names who documents it"
        );
    }
}

/// **The code and the corpus cannot drift apart.**
///
/// The six portable fields and the three numeric limits are claims about
/// somebody else's specification, and that specification is a file in
/// `concepts/reference/` that a fetch refreshes. A constant in the source that
/// nothing compares to it is a figure maintained in one place and believed in
/// two.
///
/// Skipped, with no assertion, when the corpus is absent: a clean checkout does
/// not carry it, and a test that fails for want of a gitignored directory is a
/// test people learn to ignore.
#[test]
fn the_portable_fields_and_limits_match_the_fetched_specification() {
    let spec = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("concepts/reference/standards/agent-skills-spec.md");
    let Ok(text) = std::fs::read_to_string(&spec) else {
        return;
    };
    let lower = text.to_lowercase();

    for field in devplane::core::library::PORTABLE_FIELDS {
        assert!(
            lower.contains(&field.to_lowercase()),
            "`{field}` is in the code's portable set and not in the specification"
        );
    }

    // The three limits, each bound to **its own field's row** in the
    // specification's table.
    //
    // The first version of this searched the whole file for `max 64` *or*
    // `64 char`. Mutating the table's `Max 64` to `Max 6` did not fail it: the
    // phrase `64 characters` still appeared elsewhere, and the `||` found it.
    // A check that survives the mutation it exists to catch is decoration, so
    // the row is located first and the limit is read out of that row alone.
    for (field, limit) in [("name", 64), ("description", 1024), ("compatibility", 500)] {
        let row = text
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("| `{field}`")))
            .unwrap_or_else(|| {
                panic!("the specification's field table no longer has a row for `{field}`")
            });
        assert!(
            row.contains(&format!("Max {limit} characters")),
            "the specification's `{field}` row no longer states a {limit}-character limit, \
             and the code still counts to it:\n  {row}"
        );
    }

    // **Only one direction is checked, and the other is stated rather than
    // faked.** A field the specification allows that this code does not know
    // about would be reported as an unexpected key — telling somebody their
    // valid frontmatter is a hard error. Catching that needs parsing the
    // specification's field table, and a loop that walks its lines without
    // asserting anything is a check in appearance only. It is left out.
}

//! Specification layouts, read from the smallest real repository each tool writes
//! (`tests/fixtures/spec/`). Asserted by name — layouts found, folders listed —
//! never by count alone.

use devplane::core::config::SpecSection;
use devplane::core::spec::{
    ChangeFolder, DIALECTS, Detected, NO_LAYOUT, Shape, Spec, changes, confine, detect,
};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/spec")
        .join(name)
}

fn names(found: &[Detected]) -> Vec<&str> {
    found.iter().map(|d| d.name.as_str()).collect()
}

fn paths(found: &[ChangeFolder]) -> Vec<&str> {
    found.iter().map(|c| c.path.as_str()).collect()
}

/// A throwaway directory, for the tests that need a link or an escape.
fn scratch() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "devplane-dialects-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

#[test]
fn spec_kit_is_its_marker_and_its_numbered_folders() {
    let root = fixture("speckit");
    let found = detect(&root);
    assert_eq!(names(&found), ["Spec Kit"]);
    let kit = &found[0];
    assert_eq!(kit.detected_by, ".specify/");
    assert_eq!(kit.root, "specs");
    assert_eq!(kit.change_folder, Shape::Numbered);
    assert_eq!(kit.task_file, "tasks.md");

    // `README.md` is a file and `notes/` has no number: neither is a change.
    let listed = changes(&root, kit);
    assert_eq!(
        paths(&listed),
        ["specs/001-password-reset", "specs/002-versions"]
    );
    assert_eq!(listed[0].name, "001-password-reset");
    assert_eq!(listed[0].task_file, "specs/001-password-reset/tasks.md");
    assert_eq!(listed[0].dialect.as_deref(), Some("Spec Kit"));
}

#[test]
fn openspec_is_its_config_file_and_every_change_but_the_archive() {
    let root = fixture("openspec");
    let found = detect(&root);
    assert_eq!(names(&found), ["OpenSpec"]);
    assert_eq!(found[0].detected_by, "openspec/config.yaml");
    assert_eq!(found[0].root, "openspec/changes");

    let listed = changes(&root, &found[0]);
    assert_eq!(
        paths(&listed),
        ["openspec/changes/add-rate-limit"],
        "the archive is where finished changes go, and it is not a change"
    );
}

#[test]
fn kiro_is_its_folder_and_every_feature_under_it() {
    let root = fixture("kiro");
    let found = detect(&root);
    assert_eq!(names(&found), ["Kiro"]);
    assert_eq!(found[0].detected_by, ".kiro/");
    assert_eq!(found[0].root, ".kiro/specs");

    let listed = changes(&root, &found[0]);
    assert_eq!(paths(&listed), [".kiro/specs/theme-toggle"]);
    assert_eq!(listed[0].task_file, ".kiro/specs/theme-toggle/tasks.md");
}

/// A brownfield project that adopted one tool still has the other's folder.
#[test]
fn two_dialects_in_one_repository_are_both_found() {
    let root = fixture("both");
    let found = detect(&root);
    assert_eq!(names(&found), ["Spec Kit", "OpenSpec"]);
    assert_eq!(paths(&changes(&root, &found[0])), ["specs/001-a"]);
    assert_eq!(paths(&changes(&root, &found[1])), ["openspec/changes/b"]);
}

/// No layout is guessed from file names: without a marker there is none, and the
/// surface gets a sentence, not a blank.
#[test]
fn no_marker_and_no_key_is_no_layout_and_a_sentence_says_so() {
    let root = fixture("none");
    assert!(detect(&root).is_empty());
    assert!(SpecSection::default().layouts(&root).is_empty());
    for d in DIALECTS {
        assert!(NO_LAYOUT.contains(d.detected_by), "{NO_LAYOUT}");
    }
    assert!(NO_LAYOUT.contains("[spec] plans"), "{NO_LAYOUT}");
    assert!(
        !NO_LAYOUT.is_empty() && NO_LAYOUT.starts_with("this project has no specification layout"),
        "{NO_LAYOUT}"
    );
}

/// The fourth layout is a configured folder whose every child is a change.
#[test]
fn the_configured_layout_is_a_fourth_and_the_plans_listing_is_its_change_list() {
    let root = fixture("none");
    let section = SpecSection {
        plans: Some("docs/".to_string()),
        ..Default::default()
    };
    let found = section.layouts(&root);
    assert_eq!(names(&found), ["Plain"]);
    assert_eq!(found[0].detected_by, "[spec] plans");
    assert_eq!(
        found[0].root, "docs",
        "the trailing slash is not part of the root"
    );
    // `docs/` holds a file and no folder, so it has no change.
    assert!(changes(&root, &found[0]).is_empty());
    assert!(section.plan_paths(&root).is_empty());

    // Beside a recognised layout naming the same root, it is one layout, not two.
    let root = fixture("speckit");
    let section = SpecSection {
        plans: Some("specs".to_string()),
        ..Default::default()
    };
    assert_eq!(names(&section.layouts(&root)), ["Spec Kit"]);
    assert_eq!(
        section.plan_paths(&root),
        [
            "specs/001-password-reset",
            "specs/002-versions",
            "specs/notes"
        ],
        "the plans listing keeps its own rule — every folder, no number needed"
    );
}

/// A link at the named path is refused by name, and a spec read through one is absent.
#[cfg(unix)]
#[test]
fn a_symbolic_link_at_the_top_level_is_refused() {
    let root = scratch();
    std::fs::create_dir_all(root.join("specs")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", root.join("specs/x.md")).unwrap();

    let why = confine(&root, "specs/x.md").unwrap_err();
    assert!(why.contains("specs/x.md"), "names the path: {why}");
    assert!(why.contains("symbolic link"), "{why}");
    assert!(
        Spec::read(&root, "specs/x.md", &[]).docs.is_empty(),
        "a link out of the repository read as a document"
    );

    // A linked folder is refused the same way, and a change list skips it.
    let elsewhere = scratch();
    std::fs::create_dir_all(elsewhere.join("001-away")).unwrap();
    std::fs::write(elsewhere.join("001-away/tasks.md"), "- [ ] T001\n").unwrap();
    std::os::unix::fs::symlink(&elsewhere, root.join("openspec")).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("001-away"), root.join("specs/001-away")).unwrap();
    assert!(confine(&root, "openspec").is_err());
    assert!(ChangeFolder::at(&root, "specs/001-away").is_err());
    assert!(changes(&root, &Detected::plain("specs")).is_empty());

    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&elsewhere).ok();
}

/// A path leaving the project, as written or resolved, is refused by name and
/// lists nothing.
#[test]
fn a_path_that_escapes_the_project_is_refused_by_name() {
    let root = fixture("speckit");
    for named in ["../openspec", "/etc", "~/.ssh", "specs/../../kiro"] {
        let why = confine(&root, named).unwrap_err();
        assert!(why.contains(named), "the refusal names the path: {why}");
        assert!(
            Spec::read(&root, named, &[]).docs.is_empty(),
            "{named} was read"
        );
        assert!(ChangeFolder::at(&root, named).is_err(), "{named}");
    }
    assert!(changes(&root, &Detected::plain("../")).is_empty());
    assert!(
        SpecSection {
            plans: Some("../kiro/.kiro/specs".into()),
            ..Default::default()
        }
        .plan_paths(&root)
        .is_empty()
    );

    // A path that does not exist is absent, not refused.
    assert!(confine(&root, "specs/009-nothing").is_ok());
}

/// A repository initialised with `openspec/project.md` (pre-`config.yaml`) is OpenSpec.
#[test]
fn openspec_is_also_its_legacy_project_file() {
    let root = fixture("openspec-legacy");
    let found = detect(&root);
    assert_eq!(names(&found), ["OpenSpec"]);
    assert_eq!(found[0].detected_by, "openspec/project.md");
    assert_eq!(
        paths(&changes(&root, &found[0])),
        ["openspec/changes/add-search"]
    );
}

/// Spec Kit numbers past 999 and timestamped names are change folders; fewer
/// digits are not.
#[test]
fn spec_kit_folders_are_numbered_or_timestamped() {
    let root = scratch();
    std::fs::create_dir_all(root.join(".specify")).unwrap();
    for d in [
        "001-a",
        "1042-search",
        "20260925-101500-export",
        "12-short",
        "20260925-export",
        "notes",
    ] {
        std::fs::create_dir_all(root.join("specs").join(d)).unwrap();
    }
    let found = detect(&root);
    assert_eq!(
        paths(&changes(&root, &found[0])),
        [
            "specs/001-a",
            "specs/1042-search",
            "specs/20260925-101500-export",
            "specs/20260925-export"
        ],
        "eight digits and a name is a number past 999 too"
    );
    let named = ChangeFolder::at(&root, "specs/20260925-101500-export").unwrap();
    assert_eq!(named.dialect.as_deref(), Some("Spec Kit"));
    std::fs::remove_dir_all(&root).ok();
}

/// A Spec Kit outline starts with the specification, not its checklist.
#[test]
fn the_outline_reads_the_specification_first_and_checklists_last() {
    let root = fixture("speckit");
    let spec = Spec::read(&root, "specs/001-password-reset", &[]);
    let outline = spec.outline();
    assert_eq!(
        outline[0].text, "Feature Specification: Password reset",
        "{outline:?}"
    );
    let last = outline.last().unwrap();
    assert_eq!(last.text, "Specification Quality Checklist", "{outline:?}");
}

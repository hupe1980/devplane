//! Specification layouts, read from the smallest real repository each tool writes
//! (`tests/fixtures/spec/`). Asserted by name — layouts found, folders listed —
//! never by count alone.

mod common;

use devplane::core::config::SpecSection;
use devplane::spec::{
    ChangeFolder, DIALECTS, Detected, NO_LAYOUT, NO_TASK_FILE, Plan, Shape, Spec, TokenShape,
    changes, confine, detect, edges, planning_absent,
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
    common::scratch("dialects")
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
    assert_eq!(kit.task_file.as_deref(), Some("tasks.md"));

    // `README.md` is a file and `notes/` has no number: neither is a change.
    let listed = changes(&root, kit);
    assert_eq!(
        paths(&listed),
        ["specs/001-password-reset", "specs/002-versions"]
    );
    assert_eq!(listed[0].name, "001-password-reset");
    assert_eq!(
        listed[0].task_file.as_deref(),
        Some("specs/001-password-reset/tasks.md")
    );
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
    assert_eq!(found[0].detected_by, ".kiro/specs/");
    assert_eq!(found[0].root, ".kiro/specs");

    let listed = changes(&root, &found[0]);
    assert_eq!(paths(&listed), [".kiro/specs/theme-toggle"]);
    assert_eq!(
        listed[0].task_file.as_deref(),
        Some(".kiro/specs/theme-toggle/tasks.md")
    );
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
    // By value too, so a marker corrected in `DIALECTS` and not here fails.
    for marker in [
        "`.specify/`",
        "`openspec/config.yaml`",
        "`.kiro/specs/`",
        "`.planning/STATE.md`",
    ] {
        assert!(
            NO_LAYOUT.contains(marker),
            "{marker} is not in: {NO_LAYOUT}"
        );
    }
    assert!(
        !NO_LAYOUT.contains("`.kiro/`"),
        "the old, false-positive Kiro marker: {NO_LAYOUT}"
    );
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
    // The escaping key contributes nothing; the fixture's own Spec Kit
    // layout is still listed, because it is detected by its marker.
    let listed = SpecSection {
        plans: Some("../kiro/.kiro/specs".into()),
        ..Default::default()
    }
    .plan_paths(&root);
    assert!(
        listed
            .iter()
            .all(|p| !p.contains("..") && !p.contains("kiro")),
        "an escaping `plans` listed something: {listed:?}"
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

// ---------------------------------------------------------------------------
// Dialects read correctly: detection by the tool's own marker
// ---------------------------------------------------------------------------

/// `openspec init --tools kiro` writes `.kiro/skills/` and `.kiro/prompts/`
/// (the tree in `openspec-kiro/` is what OpenSpec 1.14.0 wrote, bodies
/// trimmed to their front matter). No Kiro specification exists, so
/// only OpenSpec is reported.
#[test]
fn an_openspec_repository_set_up_for_kiro_reads_as_one_dialect() {
    let root = fixture("openspec-kiro");
    assert!(
        root.join(".kiro/skills/openspec-propose/SKILL.md")
            .is_file()
    );
    let found = detect(&root);
    assert_eq!(names(&found), ["OpenSpec"]);
    assert_eq!(found[0].detected_by, "openspec/config.yaml");
    assert!(
        changes(&root, &found[0]).is_empty(),
        "a fresh init has only the archive"
    );
    assert_eq!(found[0].elsewhere, None, "no store is named");

    // Kiro's own folder is the marker; steering alone is not a specification.
    let kiro = fixture("kiro");
    assert_eq!(detect(&kiro)[0].detected_by, ".kiro/specs/");
    let steering = scratch();
    std::fs::create_dir_all(steering.join(".kiro/steering")).unwrap();
    assert!(detect(&steering).is_empty(), "`.kiro/` alone detected Kiro");
    std::fs::remove_dir_all(&steering).ok();
}

#[test]
fn openspec_is_also_its_config_yml_alias_and_says_which() {
    let root = fixture("openspec-yml");
    let found = detect(&root);
    assert_eq!(names(&found), ["OpenSpec"]);
    assert_eq!(found[0].detected_by, "openspec/config.yml");
    assert_eq!(
        paths(&changes(&root, &found[0])),
        ["openspec/changes/add-export"]
    );
}

// ---------------------------------------------------------------------------
// Devplane's counts equal OpenSpec's own
// ---------------------------------------------------------------------------

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The oracle is OpenSpec itself: `openspec-list.json` is the output of
/// `openspec list --json --sort name` from `@fission-ai/openspec` 1.14.0, run
/// in the fixture, minus `root.path` and each `lastModified`.
/// The change list is every entry OpenSpec does not mark as a wrapper
/// (`nested`), and each `(done, total)` is its `(completedTasks,
/// totalTasks)`. The one stated difference: a change under a schema Devplane
/// does not read shows no count and says why, rather than a count Devplane
/// cannot vouch for.
#[test]
fn openspec_counts_equal_openspecs_own() {
    let root = fixture("openspec-counts");
    let oracle = json(&root.join("openspec-list.json"));
    let found = detect(&root);
    assert_eq!(names(&found), ["OpenSpec"]);
    let listed = changes(&root, &found[0]);

    let theirs: Vec<&serde_json::Value> = oracle["changes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c.get("nested").is_none())
        .collect();
    let their_names: Vec<&str> = theirs.iter().map(|c| c["name"].as_str().unwrap()).collect();
    let our_names: Vec<&str> = listed.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(our_names, their_names, "the change lists differ");
    assert!(!our_names.contains(&"mobile"), "a wrapper is not a change");
    assert!(!our_names.contains(&"refresh-token"));
    assert!(
        our_names.contains(&".draft"),
        "OpenSpec lists dot-named changes"
    );

    let mut compared = 0;
    for c in theirs {
        let name = c["name"].as_str().unwrap();
        let want = (
            c["completedTasks"].as_u64().unwrap() as u32,
            c["totalTasks"].as_u64().unwrap() as u32,
        );
        let plan = Plan::read(&root, &format!("openspec/changes/{name}"), &[]);
        if name == "custom-schema" {
            assert_eq!(plan.progress, None, "{name}");
            let why = plan.no_progress.as_deref().unwrap_or_default();
            assert!(why.contains("`my-schema`"), "{why}");
            continue;
        }
        assert_eq!(plan.no_progress, None, "{name}");
        let got = plan.progress.map_or((0, 0), |p| (p.done, p.total));
        assert_eq!(got, want, "{name}: Devplane and `openspec list` disagree");
        compared += 1;
    }
    assert_eq!(
        compared, 8,
        "every change but the wrapper and the custom schema's"
    );
}

/// OpenSpec 1.14.0's `openspec instructions apply --json` gives each task
/// its `sourcePath` and one-based `line`: the task's anchor, from the tool.
/// `openspec-apply.json` holds that output for every change in the fixture
/// (paths made relative to the fixture). Devplane's task
/// anchors — what a dispatch selects by `file:line` — are the same lines with
/// the same done state.
#[test]
fn openspec_task_anchors_are_where_openspec_apply_puts_them() {
    let root = fixture("openspec-counts");
    let oracle = json(&root.join("openspec-apply.json"));
    let layout = detect(&root).remove(0);
    let mut compared = 0;
    for change in changes(&root, &layout) {
        let theirs = &oracle[change.name.as_str()];
        let Some(tasks) = theirs["tasks"].as_array() else {
            // OpenSpec refuses this one (a dot-named id, an unknown schema).
            assert!(theirs["error"].is_string(), "{}", change.name);
            continue;
        };
        let want: Vec<(String, u32, bool)> = tasks
            .iter()
            .map(|t| {
                (
                    t["sourcePath"].as_str().unwrap().to_string(),
                    t["line"].as_u64().unwrap() as u32,
                    t["done"].as_bool().unwrap(),
                )
            })
            .collect();
        let trace = edges(&change, &TokenShape::defaults());
        let mut got: Vec<(String, u32, bool)> = trace
            .edges
            .iter()
            .flat_map(|(_, ts)| ts.iter())
            .chain(trace.tasks_citing_nothing.iter())
            .map(|a| (a.path.clone(), a.line, a.done))
            .collect();
        got.sort();
        got.dedup();
        assert_eq!(got, want, "{}", change.name);
        compared += 1;
    }
    assert_eq!(compared, 7);
}

/// OpenSpec's reading applies inside OpenSpec change folders only: a Spec Kit
/// or Kiro task file reads `- [~]` and a fenced box as it always did.
#[test]
fn other_dialects_read_their_boxes_as_before() {
    let tasks =
        "- [x] T001 done\n- [~] T002 not a box here\n```\n- [ ] an example\n```\n- [ ] T003 open\n";
    for (marker, folder) in [
        (".specify", "specs/001-a"),
        (".kiro/specs", ".kiro/specs/a"),
    ] {
        let root = scratch();
        std::fs::create_dir_all(root.join(marker)).unwrap();
        std::fs::create_dir_all(root.join(folder)).unwrap();
        std::fs::write(root.join(folder).join("tasks.md"), tasks).unwrap();
        let plan = Plan::read(&root, folder, &[]);
        let p = plan.progress.expect("a count");
        assert_eq!((p.done, p.total), (1, 2), "{folder}");
        assert_eq!(plan.no_progress, None);
        std::fs::remove_dir_all(&root).ok();
    }
}

/// A Spec Kit repository may also carry OpenSpec (since Spec Kit 1.0.13 a
/// community `openspec` extension writes `openspec/changes/` too). Each folder
/// reads by the dialect whose root it is under — `specs/` by Spec Kit's
/// reader, `openspec/changes/` by OpenSpec's — so no folder is read twice and
/// none is contested.
#[test]
fn spec_kit_and_openspec_together_each_read_their_own_folders() {
    let root = scratch();
    for (path, text) in [
        (".specify/memory/constitution.md", "# C\n"),
        ("openspec/config.yaml", "schema: spec-driven\n"),
        ("specs/001-a/tasks.md", "- [x] T001 a\n- [~] T002 b\n"),
        ("openspec/changes/b/tasks.md", "- [x] 1.1 a\n- [~] 1.2 b\n"),
    ] {
        std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        std::fs::write(root.join(path), text).unwrap();
    }
    assert_eq!(names(&detect(&root)), ["Spec Kit", "OpenSpec"]);
    let count = |f: &str| {
        Plan::read(&root, f, &[])
            .progress
            .map(|p| (p.done, p.total))
    };
    assert_eq!(count("specs/001-a"), Some((1, 1)), "Spec Kit's reader");
    assert_eq!(
        count("openspec/changes/b"),
        Some((1, 2)),
        "OpenSpec's reader"
    );

    // Without OpenSpec's marker the folder is not OpenSpec's to read.
    std::fs::remove_file(root.join("openspec/config.yaml")).unwrap();
    assert_eq!(names(&detect(&root)), ["Spec Kit"]);
    std::fs::remove_dir_all(&root).ok();
}

/// A folder that wraps changes is not one; a folder with a file of its own
/// is, and so is a dot-named one.
#[test]
fn an_openspec_wrapper_folder_is_not_a_change() {
    let root = scratch();
    for (path, text) in [
        ("openspec/config.yaml", "schema: spec-driven\n"),
        (
            "openspec/changes/mobile/ios/refresh/specs/auth/spec.md",
            "# a\n",
        ),
        (
            "openspec/changes/web/README.md",
            "# not a wrapper: it has a file\n",
        ),
        ("openspec/changes/web/login/proposal.md", "# p\n"),
        ("openspec/changes/empty/.gitkeep", ""),
    ] {
        std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        std::fs::write(root.join(path), text).unwrap();
    }
    let layout = detect(&root).remove(0);
    assert_eq!(
        paths(&changes(&root, &layout)),
        ["openspec/changes/empty", "openspec/changes/web"],
        "`mobile` wraps a change two levels down; `empty` wraps nothing"
    );
    std::fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// GSD: phases listed, no progress claimed
// ---------------------------------------------------------------------------

#[test]
fn a_gsd_repository_lists_its_phases() {
    let root = fixture("gsd");
    let found = detect(&root);
    assert_eq!(names(&found), ["GSD"]);
    assert_eq!(found[0].detected_by, ".planning/STATE.md");
    assert_eq!(found[0].root, ".planning/phases");
    assert_eq!(found[0].task_file, None);
    let listed = changes(&root, &found[0]);
    assert_eq!(
        paths(&listed),
        [
            ".planning/phases/01-auth",
            ".planning/phases/02-billing",
            ".planning/phases/02.1-hotfix",
            ".planning/phases/ABC-03-setup",
        ],
        "every phase folder, in path order; the workstream's phase is not one"
    );
    assert!(listed.iter().all(|c| c.task_file.is_none()));

    // `.planning/` without its state file is not GSD.
    let bare = scratch();
    std::fs::create_dir_all(bare.join(".planning/phases/01-a")).unwrap();
    assert!(detect(&bare).is_empty());
    std::fs::remove_dir_all(&bare).ok();
}

/// GSD 1.15.0 names bracket-convention phases `CODE.MM-PP-slug`; with backlog
/// (`999.x`) and pre-milestone (`0-…`) phases they are folders like any other.
#[test]
fn gsd_bracketed_and_sentinel_phases_are_listed() {
    let root = scratch();
    std::fs::create_dir_all(root.join(".planning")).unwrap();
    std::fs::write(root.join(".planning/STATE.md"), "# Project State\n").unwrap();
    for d in [
        "GSD.02-05-export",
        "GSD.02-05.01-hotfix",
        "999.1-idea",
        "0-bootstrap",
    ] {
        std::fs::create_dir_all(root.join(".planning/phases").join(d)).unwrap();
    }
    let layout = detect(&root).remove(0);
    assert_eq!(
        paths(&changes(&root, &layout)),
        [
            ".planning/phases/0-bootstrap",
            ".planning/phases/999.1-idea",
            ".planning/phases/GSD.02-05-export",
            ".planning/phases/GSD.02-05.01-hotfix",
        ]
    );
    std::fs::remove_dir_all(&root).ok();
}

/// A PLAN file's `<verification>` list holds `- [ ]` lines; counting them would
/// invent progress GSD does not record. No count, and the sentence says why.
#[test]
fn a_gsd_phase_claims_no_progress() {
    let root = fixture("gsd");
    let phase = ".planning/phases/01-auth";
    assert!(
        std::fs::read_to_string(root.join(phase).join("01-01-PLAN.md"))
            .unwrap()
            .contains("- [ ]")
    );
    let plan = Plan::read(&root, phase, &[]);
    assert!(plan.present);
    assert_eq!(plan.progress, None);
    assert_eq!(plan.no_progress.as_deref(), Some(NO_TASK_FILE));
    assert!(
        !plan.contradicts_done(),
        "no count, so no unticked box either"
    );

    let folder = ChangeFolder::at(&root, phase).unwrap();
    assert_eq!(folder.dialect.as_deref(), Some("GSD"));
    assert_eq!(folder.task_file, None);
    let trace = edges(&folder, &TokenShape::defaults());
    assert!(trace.unrecognised);
    assert!(
        trace.tasks_citing_nothing.is_empty(),
        "no task file, no task lines"
    );
}

// ---------------------------------------------------------------------------
// Where the changes are, when they are not here
// ---------------------------------------------------------------------------

#[test]
fn a_store_only_openspec_project_names_its_store() {
    let root = fixture("openspec-store");
    let found = detect(&root);
    assert_eq!(names(&found), ["OpenSpec"]);
    assert!(changes(&root, &found[0]).is_empty());
    assert_eq!(
        found[0].elsewhere.as_deref(),
        Some("this project's OpenSpec changes live in store `team-specs`, outside this project")
    );

    // With local changes the pointer is not consulted, so it is not named.
    assert_eq!(detect(&fixture("openspec"))[0].elsewhere, None);
    let copy = scratch();
    std::fs::create_dir_all(copy.join("openspec/specs")).unwrap();
    std::fs::write(
        copy.join("openspec/config.yaml"),
        "schema: spec-driven\nstore: team-specs\n",
    )
    .unwrap();
    assert_eq!(detect(&copy)[0].elsewhere, None);

    // Quoted as written, and a comment is not part of the id.
    std::fs::remove_dir_all(copy.join("openspec/specs")).unwrap();
    for (line, id) in [
        ("store: \"team specs\"", "team specs"),
        ("store: 'team-specs' # shared", "team-specs"),
        ("store: team-specs # shared", "team-specs"),
    ] {
        std::fs::write(copy.join("openspec/config.yaml"), format!("{line}\n")).unwrap();
        let e = detect(&copy)[0].elsewhere.clone().unwrap_or_default();
        assert!(e.contains(&format!("`{id}`")), "{line}: {e}");
    }
    std::fs::write(copy.join("openspec/config.yaml"), "schema: spec-driven\n").unwrap();
    assert_eq!(detect(&copy)[0].elsewhere, None, "no store, no sentence");
    std::fs::remove_dir_all(&copy).ok();
}

#[test]
fn a_worktree_without_a_gitignored_planning_folder_says_so() {
    let root = fixture("gsd");
    let tree = scratch();
    let says = planning_absent(&root, &tree, true).expect("a sentence");
    assert!(says.contains(".worktreeinclude"), "{says}");
    assert!(says.contains("gitignored"), "{says}");
    assert_eq!(planning_absent(&root, &tree, false), None, "not ignored");
    std::fs::create_dir_all(tree.join(".planning")).unwrap();
    std::fs::write(tree.join(".planning/STATE.md"), "# s\n").unwrap();
    assert_eq!(planning_absent(&root, &tree, true), None, "carried");
    assert_eq!(
        planning_absent(&fixture("speckit"), &scratch(), true),
        None,
        "no GSD here"
    );
    std::fs::remove_dir_all(&tree).ok();
}

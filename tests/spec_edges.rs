//! An edge is a token in a heading and again in a task line, matched by shape
//! alone. Cases assert names, not counts: a count passes with the wrong names.

use devplane::core::config::{ProjectConfig, SpecSection};
use devplane::core::run::{Observed, Run, RunMode};
use devplane::core::spec::{
    ChangeFolder, TaskAnchor, TokenShape, Trace, UNRECOGNISED, changes, counts, detect, edges,
    select_tasks, token_rows,
};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/spec")
        .join(name)
}

/// The trace of one change folder under the default shapes.
fn trace(repo: &str, change: &str) -> Trace {
    let root = fixture(repo);
    let layout = detect(&root).into_iter().next().expect("a layout");
    let change = changes(&root, &layout)
        .into_iter()
        .find(|c| c.path == change)
        .unwrap_or_else(|| panic!("{change} is not a change of {repo}"));
    edges(&change, &TokenShape::defaults())
}

fn edge_names(t: &Trace) -> Vec<(&str, Vec<&str>)> {
    t.edges
        .iter()
        .map(|(tok, tasks)| {
            (
                tok.as_str(),
                tasks.iter().map(|a| a.text.as_str()).collect(),
            )
        })
        .collect()
}

fn texts(anchors: &[TaskAnchor]) -> Vec<&str> {
    anchors.iter().map(|a| a.text.as_str()).collect()
}

/// Spec Kit's own template: `[US1]` on task lines, `FR-` requirements no task cites.
#[test]
fn spec_kit_edges_and_orphans_are_reported_by_name() {
    let t = trace("speckit", "specs/001-password-reset");
    assert!(!t.unrecognised);
    assert_eq!(
        edge_names(&t),
        [
            (
                "US1",
                vec![
                    "- [x] T002 [US1] Add the reset route in src/routes/reset.rs",
                    "- [ ] T003 [P] [US1] Send one mail per request in src/mail/send.rs",
                ]
            ),
            (
                "US2",
                vec!["- [ ] T004 [US2] Mark a used link in src/auth/reset.rs"]
            ),
        ]
    );
    assert_eq!(
        t.orphan_requirements,
        ["US3", "FR-001", "FR-002", "FR-003"],
        "a story and requirements with no task, by name — the fenced example \
         citing US3 is not a task"
    );
    assert_eq!(
        texts(&t.tasks_citing_nothing),
        [
            "- [x] T001 Create the mail template in src/mail/reset.html",
            "- [ ] T005 [US9] Rate-limit the endpoint in src/routes/reset.rs",
        ],
        "one cites nothing at all, one cites a story nothing heads"
    );
    assert!(t.tasks_citing_nothing[0].cites.is_empty());
    assert_eq!(t.tasks_citing_nothing[1].cites, ["US9"]);

    // The anchor points at the line, so a surface can.
    let (_, tasks) = &t.edges[0];
    assert_eq!(tasks[0].path, "specs/001-password-reset/tasks.md");
    assert_eq!(tasks[0].line, 13);
    assert!(tasks[0].done);
    assert!(
        !t.edges[1].1[0].done,
        "ticked is what the agent wrote, and it is carried as such"
    );

    // Selecting by the story sends its tasks.
    let sent = select_tasks(&t, "specs/001-password-reset", &["US1".to_string()]).unwrap();
    assert_eq!(sent.len(), 2);
}

/// OpenSpec deltas carry no identifiers: no edges, but tasks are still listed,
/// counted and selectable by `file:line`.
#[test]
fn openspec_has_no_identifiers_and_its_tasks_are_still_listed() {
    let t = trace("openspec", "openspec/changes/add-rate-limit");
    assert!(t.unrecognised, "{t:?}");
    assert!(t.edges.is_empty() && t.orphan_requirements.is_empty());
    assert_eq!(
        texts(&t.tasks_citing_nothing),
        [
            "- [x] 1.1 Add the limiter",
            "- [ ] 1.2 Write the retry header",
            "- [ ] 1.3 Document the limit",
        ]
    );
    let sent = select_tasks(
        &t,
        "openspec/changes/add-rate-limit",
        &["tasks.md:4".to_string()],
    )
    .unwrap();
    assert_eq!(sent[0].text, "1.2 Write the retry header");
    let whole = select_tasks(
        &t,
        "openspec/changes/add-rate-limit",
        &["openspec/changes/add-rate-limit/tasks.md:4".to_string()],
    )
    .unwrap();
    assert_eq!(whole, sent, "the path from the project root names it too");
    // A token nothing cites says why, and how to select instead.
    let why = select_tasks(&t, "openspec/changes/add-rate-limit", &["REQ-1".into()]).unwrap_err();
    assert!(why.contains("file:line"), "{why}");
    assert!(why.contains(UNRECOGNISED), "{why}");
}

/// Kiro: `_Requirements: 1.2, 2.2_` nested under a task cites by position, in
/// this dialect only.
#[test]
fn kiro_edges_and_orphans_are_reported_by_name() {
    let t = trace("kiro", ".kiro/specs/theme-toggle");
    assert!(!t.unrecognised, "{t:?}");
    assert_eq!(
        edge_names(&t),
        [
            (
                "Requirement 1",
                vec![
                    "- [ ] 1. Set up the theme context",
                    "- [ ] 2.1 Persist the choice"
                ]
            ),
            ("1.1", vec!["- [ ] 1. Set up the theme context"]),
            ("1.2", vec!["- [ ] 2.1 Persist the choice"]),
            (
                "Requirement 2",
                vec![
                    "- [ ] 2.1 Persist the choice",
                    "- [x] 2.2 Listen for the media query"
                ]
            ),
            ("2.1", vec!["- [x] 2.2 Listen for the media query"]),
            ("2.2", vec!["- [ ] 2.1 Persist the choice"]),
        ]
    );
    assert_eq!(
        t.orphan_requirements,
        ["Requirement 3", "3.1"],
        "a requirement with no task"
    );
    assert_eq!(
        texts(&t.tasks_citing_nothing),
        [
            "- [ ] 2. Persist and follow the system theme",
            "- [ ] 3. Write the end-to-end tests",
        ],
        "a parent task does not inherit its children's citations"
    );

    // Under a non-Kiro layout, `_Requirements:` lines are not citations.
    let root = std::env::temp_dir().join(format!(
        "devplane-kiro-plain-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(root.join("plans/theme")).unwrap();
    for f in ["requirements.md", "tasks.md"] {
        std::fs::copy(
            fixture("kiro").join(".kiro/specs/theme-toggle").join(f),
            root.join("plans/theme").join(f),
        )
        .unwrap();
    }
    let plain = ChangeFolder::at(&root, "plans/theme").unwrap();
    assert_eq!(plain.dialect, None);
    let t = edges(&plain, &TokenShape::defaults());
    assert!(t.unrecognised, "{t:?}");
    assert!(t.tasks_citing_nothing.iter().all(|a| a.cites.is_empty()));
    std::fs::remove_dir_all(&root).ok();
}

/// Two identical lines are two tasks.
#[test]
fn identical_task_lines_are_two_tasks() {
    let root = fixture("openspec-legacy");
    let change = ChangeFolder::at(&root, "openspec/changes/add-search").unwrap();
    assert_eq!(change.dialect.as_deref(), Some("OpenSpec"));
    let t = edges(&change, &TokenShape::defaults());
    let keys: Vec<String> = t.tasks_citing_nothing.iter().map(|a| a.key()).collect();
    assert_eq!(keys, ["Add the endpoint", "Add the endpoint #2"]);
    let c = counts(&t, &[], true, None);
    assert_eq!(c.tasks, 2);
    let sent = select_tasks(
        &t,
        "openspec/changes/add-search",
        &["tasks.md:3".to_string(), "tasks.md:4".to_string()],
    )
    .unwrap();
    assert_eq!(sent.len(), 2, "the second was collapsed into the first");
}

/// Bare-number notation gets no edges by default, and every surface (trace, plan
/// list, change view) names the key to set instead of showing zeros.
#[tokio::test]
async fn an_unrecognised_notation_names_the_key_on_every_surface() {
    use devplane::core::{Change, Project, ProjectId, World};
    let root = fixture("none");
    let t = edges(
        &ChangeFolder::at(&root, "docs").unwrap(),
        &TokenShape::defaults(),
    );
    assert!(t.unrecognised, "{t:?}");
    assert!(t.edges.is_empty() && t.tasks_citing_nothing.is_empty());
    assert!(UNRECOGNISED.contains("[spec] tokens"), "{UNRECOGNISED}");
    assert!(
        UNRECOGNISED.contains("not") || UNRECOGNISED.contains("no "),
        "{UNRECOGNISED}"
    );

    // The same folder as the surfaces read it.
    let mut world = World::new();
    world.upsert_project(Project::from_root(root.clone()));
    let project = ProjectId::from_path(&root);
    let mut change = Change::new(project, "notes".into(), "…".into());
    change.spec = Some("docs".into());
    let db = std::env::temp_dir().join(format!("vp-edges-{}.db", uuid::Uuid::new_v4().simple()));
    let store = devplane::store::Store::open(&db).await.unwrap();
    let snap = devplane::view::Snapshot {
        world,
        changes: vec![change],
        open_asks: Vec::new(),
        live: Default::default(),
        forge: None,
        gate: None,
        broken_configs: Vec::new(),
        unwritten: (0, 0, None),
        leaked_agents: Vec::new(),
        agents: Vec::new(),
        from_host: false,
        now: jiff::Timestamp::now(),
        started_at: None,
    };
    let specs = devplane::view::specs(&snap);
    let plan = &specs["projects"][0]["plans"][0];
    assert_eq!(plan["plan"]["path"], "docs", "{specs}");
    assert_eq!(plan["trace"]["unrecognised"], true, "{plan}");
    assert!(
        plan["counts"].is_null(),
        "no edges means no counts, not zeros: {plan}"
    );
    // The change view (`devplane change show`) carries the sentence in place of counts.
    let rows = devplane::view::change_list(&snap, &store).await;
    let row = serde_json::to_value(&rows[0]).unwrap();
    assert!(row["counts"].is_null(), "{row}");
    assert_eq!(row["counts_says"], UNRECOGNISED);
    std::fs::remove_file(&db).ok();
}

/// A run that was sent `sent`, closed at `closed`, having read `ticked`.
fn closed_run(id: &str, trace: &Trace, selectors: &[&str], closed: &str) -> Run {
    let sent = select_tasks(
        trace,
        "specs/007-counts",
        &selectors.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
    .unwrap();
    let mut r = Run::new(
        devplane::core::SessionId::new(id),
        PathBuf::from("/repo"),
        RunMode::Driven,
        "echo",
    );
    r.sent = Some(sent);
    r.observed = Some(Observed {
        fingerprint: Some("f".into()),
        ticked: Vec::new(),
        changed_at: None,
        at: closed.parse().unwrap(),
    });
    r
}

/// Ticked and verified are separate counts (11 and 9 here), never combined.
#[test]
fn ticked_and_verified_are_two_columns() {
    let t = trace("counts", "specs/007-counts");
    let a = closed_run("a", &t, &["FR-001", "FR-002"], "2026-09-25T10:00:00Z");
    let b = closed_run("b", &t, &["FR-003"], "2026-09-25T10:05:00Z");
    let pass = Some("2026-09-25T10:10:00Z".parse().unwrap());

    let c = counts(&t, &[&a, &b], true, pass);
    assert_eq!((c.tasks, c.ticked, c.verified_tasks), (15, 11, Some(9)));
    assert_eq!(c.says(false), "11 ticked · 9 verified");

    // Every box ticked and no gate run: verified is zero and says so.
    let c = counts(&t, &[&a, &b], true, None);
    assert_eq!(c.verified_tasks, Some(0));
    assert_eq!(c.says(false), "11 ticked · 0 verified");

    // No gates: absent, which is a different fact from zero.
    let c = counts(&t, &[&a, &b], false, pass);
    assert_eq!(c.verified_tasks, None);
    assert_eq!(c.says(false), "11 ticked · no gates declared");

    // Per token too: two integers or null on the wire, never a derived figure.
    let rows = token_rows(&t, &[&a, &b], true, pass);
    assert_eq!(rows.len(), 4);
    assert_eq!(
        (
            rows[3].token.as_str(),
            rows[3].tasks,
            rows[3].ticked,
            rows[3].verified_tasks
        ),
        ("FR-004", 4, 2, Some(0))
    );
    let json = serde_json::to_value(counts(&t, &[&a, &b], true, pass)).unwrap();
    for (key, value) in json.as_object().unwrap() {
        assert!(
            value.is_u64() || value.is_array() || value.is_null(),
            "Counts.{key} is not an integer, a list or absent: {value}"
        );
    }
    assert!(
        json.get("verified").is_some(),
        "the wire keeps the contract's word"
    );
}

/// A box ticked that no run was sent is listed by name.
#[test]
fn a_box_ticked_that_nobody_was_sent_is_visible() {
    let t = trace("counts", "specs/007-counts");
    let a = closed_run(
        "a",
        &t,
        &["FR-001", "FR-002", "FR-003"],
        "2026-09-25T10:00:00Z",
    );
    let c = counts(&t, &[&a], true, None);
    assert_eq!(
        c.ticked_unsent,
        [
            "T010 Write the docs (FR-004)",
            "T011 Announce the release (FR-004)"
        ]
    );
    // Sent to a run that closed with them unticked: the other gap.
    assert_eq!(
        c.sent_unticked,
        [
            "T001 Scaffold the module (FR-001)",
            "T002 Wire the route (FR-001)",
            "T003 Validate the input (FR-001)",
            "T004 Store the record (FR-002)",
            "T005 Read the record back (FR-002)",
            "T006 Expire old records (FR-002)",
            "T007 Render the list (FR-003)",
            "T008 Render one item (FR-003)",
            "T009 Paginate the list (FR-003)",
            "T012 Cache the list (FR-003)",
            "T015 Remove the flag (FR-001)",
        ],
        "the run's close recorded no ticks, so every sent task is listed"
    );
    // Nothing sent: everything ticked is ticked by hand.
    let none = counts(&t, &[], true, None);
    assert_eq!(none.ticked_unsent.len(), 11);
    assert!(none.sent_unticked.is_empty());
}

/// A version, a timeout, a figure and a release tag beside one real identifier:
/// the default shapes match only the identifier.
#[test]
fn a_bare_number_shape_would_match_five_things_and_the_default_matches_one() {
    let t = trace("speckit", "specs/002-versions");
    assert!(!t.unrecognised);
    let mut seen: Vec<&str> = t.edges.iter().map(|(tok, _)| tok.as_str()).collect();
    seen.extend(t.orphan_requirements.iter().map(String::as_str));
    for a in &t.tasks_citing_nothing {
        seen.extend(a.cites.iter().map(String::as_str));
    }
    assert_eq!(seen, ["FR-001"], "exactly one token in the whole folder");
    assert_eq!(
        edge_names(&t),
        [(
            "FR-001",
            vec!["- [ ] T003 Raise the floor from 9.9 (FR-001)"]
        )]
    );
    assert_eq!(
        texts(&t.tasks_citing_nothing),
        [
            "- [x] T001 Bump the crate to 0.8.4",
            "- [ ] T002 Keep the gate under 1.5s",
            "- [ ] T004 Read against v2.1.273",
        ]
    );
}

/// A token in another change is another change's: scope is the folder.
#[test]
fn tokens_are_scoped_to_one_change_folder() {
    // Both folders head `FR-001`; only `002-versions` cites it.
    let one = trace("speckit", "specs/001-password-reset");
    let two = trace("speckit", "specs/002-versions");
    assert!(one.orphan_requirements.contains(&"FR-001".to_string()));
    assert_eq!(two.edges[0].0, "FR-001");
    assert_eq!(two.edges[0].1.len(), 1);
    assert!(two.edges[0].1[0].path.starts_with("specs/002-versions/"));
}

/// The key replaces the defaults; an invalid prefix is reported, not matched loosely.
#[test]
fn configured_shapes_replace_the_defaults_and_a_numeric_prefix_is_refused() {
    let root = fixture("speckit");
    let change = ChangeFolder::at(&root, "specs/002-versions").unwrap();

    // Replaced, not added to: `FR-` is gone, so nothing in the folder matches.
    let only_req = SpecSection {
        tokens: Some(vec!["REQ-".to_string()]),
        ..Default::default()
    };
    assert!(only_req.trace(&change).unrecognised);
    assert_eq!(only_req.token_shapes(), [TokenShape::new("REQ-").unwrap()]);

    // Absent means the defaults.
    assert_eq!(
        SpecSection::default().token_shapes(),
        TokenShape::defaults()
    );

    // A digit in a prefix, an empty prefix and an empty list are each reported by key.
    let cfg: ProjectConfig = toml::from_str("[spec]\ntokens = [\"1.\", \"FR-\", \" \"]\n").unwrap();
    let problems = cfg.validate();
    let spec_problems: Vec<_> = problems
        .iter()
        .filter(|p| p.where_ == "[spec] tokens")
        .collect();
    assert_eq!(spec_problems.len(), 2, "{problems:?}");
    assert!(spec_problems.iter().all(|p| p.fatal));
    assert!(spec_problems[0].what.contains("1."), "{}", spec_problems[0]);
    // The good one is still in force while the bad ones are dropped.
    assert_eq!(cfg.spec.token_shapes(), [TokenShape::new("FR-").unwrap()]);

    let empty: ProjectConfig = toml::from_str("[spec]\ntokens = []\n").unwrap();
    assert!(
        empty
            .validate()
            .iter()
            .any(|p| p.where_ == "[spec] tokens" && p.fatal),
        "an empty list matches nothing and must say so"
    );
    assert!(
        ProjectConfig::default()
            .validate()
            .iter()
            .all(|p| p.where_ != "[spec] tokens")
    );
}

/// No percentage, ratio or coverage figure is computed in the reader, checked over
/// its source. The file must exist and be non-trivial, or a rename passes this.
#[test]
fn the_reader_computes_no_percentage_ratio_or_coverage() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/spec.rs");
    let source = std::fs::read_to_string(&path).expect("src/core/spec.rs is the reader");
    assert!(
        source.contains("pub fn edges"),
        "the file was found but is not the reader"
    );
    for f in ["pub fn counts", "pub fn token_rows"] {
        assert!(
            source.contains(f),
            "the counts moved out of the reader: {f}"
        );
    }

    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut hits = Vec::new();
    for (n, line) in source.lines().enumerate() {
        // A comment may quote the forbidden word to say why it is forbidden.
        let code = line.split("//").next().unwrap_or("");
        if code.contains('%') {
            hits.push(format!("{}: `%` — {}", n + 1, code.trim()));
        }
        // Division or a float cast is a fraction one keystroke away. rustfmt spaces
        // binary `/`, so path separators in strings are not caught.
        if code.contains(" / ") {
            hits.push(format!("{}: a division — {}", n + 1, code.trim()));
        }
        for cast in ["as f32", "as f64"] {
            if code.contains(cast) {
                hits.push(format!("{}: `{cast}` — {}", n + 1, code.trim()));
            }
        }
        for word in ["percent", "percentage", "ratio", "coverage"] {
            let mut from = 0;
            while let Some(i) = code[from..].find(word) {
                let at = from + i;
                let before = code[..at].chars().next_back().is_none_or(|c| !is_word(c));
                let after = code[at + word.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !is_word(c));
                if before && after {
                    hits.push(format!("{}: `{word}` — {}", n + 1, code.trim()));
                }
                from = at + word.len();
            }
        }
    }
    assert!(
        hits.is_empty(),
        "the reader names a figure it may never compute:\n{}",
        hits.join("\n")
    );

    // Every field of a trace is a Vec or a bool: no number crosses to a surface.
    let trace = Trace::default();
    let json = serde_json::to_value(&trace).unwrap();
    for (key, value) in json.as_object().unwrap() {
        assert!(
            value.is_array() || value.is_boolean(),
            "Trace.{key} is neither a list nor a flag: {value}"
        );
    }
}

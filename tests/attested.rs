//! The attested certificate: an in-toto Statement v1 with the Test Result
//! predicate, which a verifier that has never heard of Devplane reads, and the
//! person's own SSH signature over it, which a stranger checks with
//! `ssh-keygen` alone. The settlement: an exported statement validates as
//! in-toto v1; `ssh-keygen -Y verify` accepts a signed one from a plain shell;
//! one changed byte fails it.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{git, scratch};
use devplane::core::Authority;
use devplane::core::attest;
use devplane::core::certificate::{Certificate, NO_STATEMENT};
use devplane::core::change::{
    Change, CommandResult, CommitStamp, Completion, Declared, GateReport, Outcome, Reach,
};
use devplane::core::review::{WeakKind, Weakened};
use serde_json::{Value, json};

// ── A test-only walker over the JSON Schema subset the two schemas use ─────

fn schema(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/in-toto")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(&p).expect("the schema file")).unwrap()
}

/// The patterns the schemas use, each as a function. An unknown pattern
/// panics: a guard that cannot check something must not pass it.
fn matches_pattern(pattern: &str, s: &str) -> bool {
    let hex = |s: &str| s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    match pattern {
        "^[0-9a-f]{40}([0-9a-f]{24})?$" => matches!(s.len(), 40 | 64) && hex(s),
        "^[0-9a-f]{64}$" => s.len() == 64 && hex(s),
        "^[0-9a-f]+$" => !s.is_empty() && hex(s),
        "^[a-z][a-z0-9+.-]*:" => {
            let scheme = s.split(':').next().unwrap_or("");
            s.contains(':')
                && scheme.starts_with(|c: char| c.is_ascii_lowercase())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+.-".contains(c))
        }
        other => panic!("the walker does not know the pattern {other}"),
    }
}

fn type_ok(t: &str, v: &Value) -> bool {
    match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "boolean" => v.is_boolean(),
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "null" => v.is_null(),
        other => panic!("the walker does not know the type {other}"),
    }
}

/// Every violation of `s` by `v`, with where.
fn walk(root: &Value, s: &Value, v: &Value, at: &str, errs: &mut Vec<String>) {
    let Some(obj) = s.as_object() else { return };
    for key in obj.keys() {
        assert!(
            [
                "$schema",
                "$comment",
                "title",
                "$defs",
                "$ref",
                "type",
                "required",
                "const",
                "enum",
                "properties",
                "additionalProperties",
                "items",
                "minItems",
                "minProperties",
                "pattern",
                "anyOf",
                "allOf",
            ]
            .contains(&key.as_str()),
            "the walker does not know the keyword {key}"
        );
    }
    if let Some(r) = obj.get("$ref").and_then(Value::as_str) {
        let name = r.strip_prefix("#/$defs/").expect("a local $ref");
        walk(root, &root["$defs"][name], v, at, errs);
    }
    if let Some(t) = obj.get("type").and_then(Value::as_str)
        && !type_ok(t, v)
    {
        errs.push(format!("{at}: not a {t}"));
        return;
    }
    if let Some(c) = obj.get("const")
        && v != c
    {
        errs.push(format!("{at}: not {c}"));
    }
    if let Some(e) = obj.get("enum").and_then(Value::as_array)
        && !e.contains(v)
    {
        errs.push(format!("{at}: {v} not one of {e:?}"));
    }
    if let Some(p) = obj.get("pattern").and_then(Value::as_str)
        && let Some(s) = v.as_str()
        && !matches_pattern(p, s)
    {
        errs.push(format!("{at}: {s:?} does not match {p}"));
    }
    if let Some(o) = v.as_object() {
        for r in obj
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let r = r.as_str().unwrap();
            if !o.contains_key(r) {
                errs.push(format!("{at}: missing {r}"));
            }
        }
        if let Some(n) = obj.get("minProperties").and_then(Value::as_u64)
            && (o.len() as u64) < n
        {
            errs.push(format!("{at}: fewer than {n} properties"));
        }
        let props = obj.get("properties").and_then(Value::as_object);
        for (k, child) in o {
            match props.and_then(|p| p.get(k)) {
                Some(ps) => walk(root, ps, child, &format!("{at}.{k}"), errs),
                None => {
                    if let Some(ap) = obj.get("additionalProperties") {
                        walk(root, ap, child, &format!("{at}.{k}"), errs);
                    }
                }
            }
        }
    }
    if let Some(a) = v.as_array() {
        if let Some(n) = obj.get("minItems").and_then(Value::as_u64)
            && (a.len() as u64) < n
        {
            errs.push(format!("{at}: fewer than {n} items"));
        }
        if let Some(items) = obj.get("items") {
            for (i, child) in a.iter().enumerate() {
                walk(root, items, child, &format!("{at}[{i}]"), errs);
            }
        }
    }
    for sub in obj
        .get("allOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        walk(root, sub, v, at, errs);
    }
    if let Some(any) = obj.get("anyOf").and_then(Value::as_array) {
        let ok = any.iter().any(|sub| {
            let mut e = Vec::new();
            walk(root, sub, v, at, &mut e);
            e.is_empty()
        });
        if !ok {
            errs.push(format!("{at}: matches none of anyOf"));
        }
    }
}

/// The violations of the Statement v1 schema and, beneath it, the Test Result
/// predicate schema.
fn violations(st: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    let s = schema("statement-v1.schema.json");
    walk(&s, &s, st, "$", &mut errs);
    if st["predicateType"] == attest::TEST_RESULT_TYPE {
        let p = schema("test-result-v0.1.schema.json");
        walk(&p, &p, &st["predicate"], "$.predicate", &mut errs);
    } else {
        errs.push("$.predicateType: not the Test Result predicate".into());
    }
    errs
}

fn assert_valid(st: &Value) {
    let errs = violations(st);
    assert!(errs.is_empty(), "{errs:#?}\n{st:#}");
}

fn hand_written() -> Value {
    json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{ "name": "x", "digest": { "gitTree": "a".repeat(40) } }],
        "predicateType": attest::TEST_RESULT_TYPE,
        "predicate": {
            "result": "PASSED",
            "configuration": [{ "name": "c", "digest": { "sha256": "b".repeat(64) } }],
            "passedTests": ["cargo test"],
        },
    })
}

/// A guard that finds nothing to fail must itself fail: each planted fault is
/// caught, and a hand-written valid statement passes.
#[test]
fn the_walker_rejects_planted_invalid_statements() {
    assert_valid(&hand_written());
    let mut plants: Vec<(&str, Value)> = Vec::new();
    let mut v = hand_written();
    v["subject"][0]["digest"] = json!({});
    plants.push(("an empty digest", v));
    let mut v = hand_written();
    v["subject"] = json!([]);
    plants.push(("no subject", v));
    let mut v = hand_written();
    v["subject"][0].as_object_mut().unwrap().remove("digest");
    plants.push(("a subject with no digest", v));
    let mut v = hand_written();
    v["subject"][0]["digest"]["gitTree"] = json!("tree-of-abc");
    plants.push(("a tree that is no object id", v));
    let mut v = hand_written();
    v["predicate"]["result"] = json!("OK");
    plants.push(("result OK", v));
    let mut v = hand_written();
    v["predicate"]
        .as_object_mut()
        .unwrap()
        .remove("configuration");
    plants.push(("no configuration", v));
    let mut v = hand_written();
    v["_type"] = json!("https://in-toto.io/Statement/v0.1");
    plants.push(("another _type", v));
    let mut v = hand_written();
    v["predicate"]["passedTests"] = json!([1]);
    plants.push(("a test name that is not a string", v));
    for (what, st) in plants {
        assert!(
            !violations(&st).is_empty(),
            "{what} passed the schema: {st}"
        );
    }
}

// ── Fixtures ───────────────────────────────────────────────────────────────

const COMMIT: &str = "4f2a9c1e0b7d3a5c9e1f2a4b6c8d0e2f4a6b8c0d";
const TREE: &str = "d0c8b6a4f2e0d8c6b4a2f1e9c5a3d7b0e1c9a2f4";

fn stamp(clean: bool) -> CommitStamp {
    CommitStamp {
        commit: Some(COMMIT.into()),
        tree: Some(TREE.into()),
        tree_unknown: None,
        branch: Some("change/rate-limiting".into()),
        clean,
        changed_files: if clean { 0 } else { 3 },
        reach: Reach::Remote,
        remote: Some("git@github.com:acme/widgets.git".into()),
    }
}

fn report(commands: Vec<CommandResult>, commit: Option<CommitStamp>) -> GateReport {
    GateReport {
        junit: Vec::new(),
        base_commit: None,
        gate: "check".into(),
        at: "2026-10-02T10:00:00Z".parse().unwrap(),
        duration_ms: 10,
        commands,
        attempt: 1,
        spec: None,
        commit,
    }
}

fn change_with(commands: Vec<CommandResult>, commit: Option<CommitStamp>, finish: bool) -> Change {
    let mut w = Change::new(
        devplane::core::ids::ProjectId::new("p1"),
        "Add rate limiting".into(),
        "do the thing".into(),
    );
    let now = commit.clone();
    w.gates.push(report(commands, commit));
    if finish {
        let declared = declared(&w);
        w.completion = Some(Completion::of(
            &w,
            Declared::Checks(&declared),
            now.as_ref(),
        ));
    }
    w
}

fn declared(w: &Change) -> Vec<String> {
    w.check_report()
        .map(|r| r.commands.iter().map(|c| c.command.clone()).collect())
        .unwrap_or_default()
}

fn cert(w: &Change) -> Certificate<'_> {
    let d = declared(w);
    let mut c = Certificate::of(w, Declared::Checks(&d), None);
    c.weakened = Some(Vec::new());
    c.decided_by = Some(BTreeMap::new());
    c
}

fn with(outcome: Outcome) -> CommandResult {
    CommandResult {
        outcome,
        ..CommandResult::exited("cargo test", 0)
    }
}

fn skip_row(i: usize) -> Weakened {
    Weakened {
        path: format!("tests/login{i}.rs"),
        why: "adds a skip marker".into(),
        kind: WeakKind::Skip,
        matched: "#[ignore]".into(),
    }
}

/// Every shape the suite's changes take: passing, failing, timed out, never
/// started, unknown, unfinished, clean and dirty, weakened.
fn fixtures() -> Vec<(&'static str, Change)> {
    let ok = || CommandResult::exited("cargo test", 0);
    vec![
        ("passing", change_with(vec![ok()], Some(stamp(true)), true)),
        (
            "failing",
            change_with(
                vec![ok(), CommandResult::exited("cargo clippy", 1)],
                Some(stamp(true)),
                true,
            ),
        ),
        (
            "timed out",
            change_with(
                vec![with(Outcome::TimedOut { after_secs: 600 })],
                Some(stamp(true)),
                true,
            ),
        ),
        (
            "never started",
            change_with(
                vec![with(Outcome::NeverStarted {
                    reason: "no such file".into(),
                })],
                Some(stamp(true)),
                true,
            ),
        ),
        (
            "unknown",
            change_with(
                vec![with(Outcome::Unknown {
                    reason: "killed by a signal".into(),
                })],
                Some(stamp(true)),
                true,
            ),
        ),
        (
            "unfinished",
            change_with(vec![ok()], Some(stamp(true)), false),
        ),
        ("dirty", change_with(vec![ok()], Some(stamp(false)), true)),
    ]
}

// ── User story 1: a statement a standard verifier reads ────────────────────

#[test]
fn every_fixture_statement_validates_and_a_planted_one_does_not() {
    for (name, w) in fixtures() {
        let st = cert(&w)
            .statement()
            .unwrap_or_else(|e| panic!("{name}: {}", e.says()));
        assert_valid(&st);
        assert_eq!(st["predicateType"], attest::TEST_RESULT_TYPE, "{name}");
        assert_eq!(st["subject"].as_array().unwrap().len(), 1, "{name}");
        assert_eq!(st["subject"][0]["digest"]["gitTree"], TREE, "{name}");
    }
    // Weakened, with every row seen.
    let w = change_with(
        vec![CommandResult::exited("cargo test", 0)],
        Some(stamp(true)),
        true,
    );
    let mut c = cert(&w);
    c.weakened = Some(vec![skip_row(0)]);
    c.weakened_read = true;
    assert_valid(&c.statement().unwrap());

    // `devplane verify`'s statement is the same kind.
    let r = report(
        vec![CommandResult::exited("cargo test", 0)],
        Some(stamp(false)),
    );
    let st = devplane::core::certificate::verify_statement(
        &devplane::core::certificate::VerifyEvidence {
            name: "widgets",
            report: &r,
            weakened: &[skip_row(1)],
            verify: json!({ "base": "main" }),
            base_checks: json!({ "absent": "check_not_run" }),
            config_blob: Some(COMMIT),
        },
    )
    .unwrap();
    assert_valid(&st);
    assert_eq!(st["predicate"]["devplane"]["verify"]["base"], "main");
    assert_eq!(
        st["predicate"]["configuration"][1]["digest"]["gitBlob"],
        COMMIT
    );

    // Planted: a statement whose subject digest is gone fails.
    let mut planted = cert(&fixtures()[0].1).statement().unwrap();
    planted["subject"][0]["digest"] = json!({});
    assert!(!violations(&planted).is_empty());
}

#[test]
fn result_is_check_exit_codes_and_nothing_else() {
    for (name, w) in fixtures() {
        let st = cert(&w).statement().unwrap();
        let r = w.check_report().unwrap();
        let all_zero = r
            .commands
            .iter()
            .all(|c| matches!(c.outcome, Outcome::Exited { code: 0 }));
        let want = if all_zero { "PASSED" } else { "FAILED" };
        assert_eq!(st["predicate"]["result"], want, "{name}");
        let p = &st["predicate"];
        let mut both: Vec<&str> = p["passedTests"]
            .as_array()
            .unwrap()
            .iter()
            .chain(p["failedTests"].as_array().unwrap())
            .map(|v| v.as_str().unwrap())
            .collect();
        let ran: Vec<&str> = r.commands.iter().map(|c| c.command.as_str()).collect();
        both.sort_unstable();
        let mut sorted = ran.clone();
        sorted.sort_unstable();
        assert_eq!(
            both, sorted,
            "{name}: passed and failed are the commands, disjoint"
        );
        assert!(!st.to_string().contains("WARNED"), "{name}");
    }
    // A weakened check whose `check` passed is still PASSED.
    let w = change_with(
        vec![CommandResult::exited("cargo test", 0)],
        Some(stamp(true)),
        true,
    );
    let mut c = cert(&w);
    c.weakened = Some(vec![skip_row(0)]);
    assert_eq!(c.statement().unwrap()["predicate"]["result"], "PASSED");
}

#[test]
fn the_gate_definition_digest_recomputes_in_a_plain_shell() {
    let w = change_with(
        vec![
            CommandResult::exited("cargo test", 0),
            CommandResult::exited("sh -c 'echo it'\"'\"'s here'", 0),
        ],
        Some(stamp(true)),
        true,
    );
    let st = cert(&w).statement().unwrap();
    let c0 = &st["predicate"]["configuration"][0];
    assert_eq!(c0["name"], attest::DEFINITION_NAME);
    let line = c0["annotations"]["recompute"].as_str().unwrap();
    let out = Command::new("sh").arg("-c").arg(line).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        printed.split_whitespace().next().unwrap(),
        c0["digest"]["sha256"].as_str().unwrap(),
        "{line}"
    );
}

#[test]
fn not_attestable_is_an_exit_and_never_an_empty_subject() {
    let ok = || CommandResult::exited("cargo test", 0);
    let no_tree = {
        let mut s = stamp(true);
        s.tree = None;
        s.tree_unknown = Some("`check` moved the tree: coverage.xml".into());
        s
    };
    let not_hex = {
        let mut s = stamp(true);
        s.tree = Some("tree-of-4f2a".into());
        s
    };
    let no_gate = Change::new(
        devplane::core::ids::ProjectId::new("p1"),
        "x".into(),
        "y".into(),
    );
    let mut missing = change_with(vec![ok()], Some(stamp(true)), true);
    missing.gates.clear();
    let cases = [
        (
            change_with(vec![ok()], None, true),
            "no gate ran in a repository",
        ),
        (change_with(vec![ok()], Some(no_tree), true), "coverage.xml"),
        (
            change_with(vec![ok()], Some(not_hex), true),
            "not a git object id",
        ),
        (no_gate, "no `check` gate has run"),
        (missing, "no longer in the record"),
        (
            change_with(
                vec![CommandResult::exited("a\0b", 0)],
                Some(stamp(true)),
                true,
            ),
            "NUL",
        ),
    ];
    for (w, says) in cases {
        let c = cert(&w);
        let why = c.statement().expect_err("no statement").says();
        assert!(why.starts_with("No in-toto statement"), "{why}");
        assert!(why.contains(says), "{why}");
        let j = c.json();
        assert!(
            j.get("_type").is_none() && j.get("subject").is_none(),
            "{j}"
        );
        assert_eq!(j[NO_STATEMENT], why.as_str());
    }
}

#[test]
fn the_statement_says_it_is_exit_codes_not_a_verdict_and_exports_the_same_bytes_twice() {
    let w = change_with(
        vec![CommandResult::exited("cargo test", 0)],
        Some(stamp(true)),
        true,
    );
    let a = attest::serialize(&cert(&w).statement().unwrap());
    let b = attest::serialize(&cert(&w).statement().unwrap());
    assert_eq!(a, b);
    assert!(a.ends_with(b"}\n") && !a.ends_with(b"\n\n"));
    let st: Value = serde_json::from_slice(&a).unwrap();
    let says = st["predicate"]["devplane"]["says"].as_str().unwrap();
    assert!(says.starts_with(attest::SAYS), "{says}");
    assert!(says.contains(devplane::core::certificate::LIMITS));
    assert!(says.contains("does not say the change is correct, safe or good"));
    assert_eq!(st["predicate"]["devplane"]["signed"], false);
    // Through the wire and back, as the CLI receives it: the same bytes.
    let round: Value = serde_json::from_str(&st.to_string()).unwrap();
    assert_eq!(attest::serialize(&round), a);
}

// ── User story 3: the facts in-toto has no slot for ────────────────────────

#[test]
fn weakened_checks_are_rows_or_unknown_never_empty() {
    let w = change_with(
        vec![CommandResult::exited("cargo test", 0)],
        Some(stamp(true)),
        true,
    );
    let mut c = cert(&w);
    c.weakened = Some(vec![skip_row(0)]);
    c.weakened_read = true;
    let st = c.statement().unwrap();
    let a = &st["predicate"]["devplane"]["checksAltered"];
    assert_eq!(a["known"], true);
    assert_eq!(a["allSeen"], true);
    assert_eq!(
        a["rows"][0],
        json!({ "path": "tests/login0.rs", "kind": "skip", "matched": "#[ignore]",
                "why": "adds a skip marker" })
    );

    let mut c = cert(&w);
    c.weakened = None;
    c.weakened_unreadable = Some("git exited 128".into());
    let a = &c.statement().unwrap()["predicate"]["devplane"]["checksAltered"];
    assert_eq!(a["known"], false);
    assert!(a.get("rows").is_none(), "{a}");
    assert!(a["says"].as_str().unwrap().contains("unknown"), "{a}");

    let mut c = cert(&w);
    c.weakened = Some((0..250).map(skip_row).collect());
    let a = &c.statement().unwrap()["predicate"]["devplane"]["checksAltered"];
    assert_eq!(a["rows"].as_array().unwrap().len(), 200);
    assert_eq!(a["omitted"], 50);
}

#[tokio::test]
async fn permissions_are_counted_per_authority_only() {
    use devplane::core::{Decision, ids::RunId};
    let dir = scratch("attest-counts");
    let store = devplane::store::Store::open(&dir.join("devplane.db"))
        .await
        .unwrap();
    let w = change_with(
        vec![CommandResult::exited("cargo test", 0)],
        Some(stamp(true)),
        true,
    );
    let run = RunId::new("s1");
    let other = RunId::new("s2");
    let add = |a: Authority, run: &RunId, tool: bool| {
        let d = Decision::new(a, "agent:tool.use", "pnpm test", "allow").for_run(run);
        match tool {
            true => d.by_tool("Bash"),
            false => d,
        }
    };
    for _ in 0..3 {
        store
            .append_decision(&add(Authority::Person, &run, true))
            .await
            .unwrap();
    }
    for _ in 0..12 {
        store
            .append_decision(&add(Authority::Rule, &run, true))
            .await
            .unwrap();
    }
    store
        .append_decision(&add(Authority::Nobody, &run, true).for_change(&w.id))
        .await
        .unwrap();
    // Devplane's own act names no tool; another change's run is not this one.
    store
        .append_decision(&add(Authority::Devplane, &run, false))
        .await
        .unwrap();
    store
        .append_decision(&add(Authority::Person, &other, true))
        .await
        .unwrap();

    let counts = store.authority_counts(&w.id, &[run]).await.unwrap();
    let mut c = cert(&w);
    c.decided_by = Some(counts);
    let st = c.statement().unwrap();
    assert_eq!(
        st["predicate"]["devplane"]["permissionsDecidedBy"],
        json!({ "nobody": 1, "person": 3, "rule": 12 })
    );
    let text = st.to_string().to_lowercase();
    for tell in [
        "rate",
        "duration",
        "per_person",
        "streak",
        "score",
        "confidence",
        "percent",
    ] {
        assert!(
            !st["predicate"]["devplane"]["permissionsDecidedBy"]
                .to_string()
                .contains(tell),
            "{tell}"
        );
        let _ = &text;
    }
    // Not read is said, never `{}`.
    let mut c = cert(&w);
    c.decided_by = None;
    let st = c.statement().unwrap();
    assert!(
        st["predicate"]["devplane"]
            .get("permissionsDecidedBy")
            .is_none()
    );
    assert!(st["predicate"]["devplane"]["permissionsUnknown"].is_string());
}

#[test]
fn the_extension_changes_no_other_field() {
    let standard = |st: &Value| {
        let p = &st["predicate"];
        json!([
            st["subject"],
            p["result"],
            p["configuration"],
            p["passedTests"],
            p["failedTests"]
        ])
    };
    for (name, w) in fixtures() {
        let bare = cert(&w).statement().unwrap();
        let mut c = cert(&w);
        c.weakened = Some((0..5).map(skip_row).collect());
        c.weakened_read = true;
        c.decided_by = Some(BTreeMap::from([(Authority::Person, 2)]));
        let full = c.statement().unwrap();
        assert_eq!(standard(&bare), standard(&full), "{name}");
        // Removing the extension leaves the standard part a valid statement.
        let mut stripped = full.clone();
        stripped["predicate"]
            .as_object_mut()
            .unwrap()
            .remove("devplane");
        assert_valid(&stripped);
        assert_eq!(standard(&stripped), standard(&full));
    }
}

#[test]
fn sibling_facts_are_carried_verbatim_or_absent() {
    use devplane::core::certificate::{base_checks_absent, base_checks_json};
    let w = change_with(
        vec![CommandResult::exited("cargo test", 0)],
        Some(stamp(true)),
        true,
    );
    let st = cert(&w).statement().unwrap();
    let ext = &st["predicate"]["devplane"];
    assert_eq!(
        ext["baseChecks"],
        base_checks_json(None, base_checks_absent(None))
    );
    assert!(!ext["baseChecks"].is_null());
    assert!(ext.get("gateInputs").is_none(), "absent, not empty: {ext}");
    // No key Devplane names carries `$`; dots only in the OpenTelemetry key.
    fn keys(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                for (k, c) in o {
                    out.push(k.clone());
                    keys(c, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|c| keys(c, out)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    keys(ext, &mut all);
    for k in all {
        assert!(!k.contains('$'), "{k}");
        assert!(!k.contains('.') || k == "gen_ai.evidence.origin", "{k}");
    }
}

// ── User story 2: the person's signature, checked without Devplane ─────────

/// `ssh-keygen` with `-Y` (OpenSSH 8.0 or later). Missing fails the test on
/// macOS and Linux; Windows runners' OpenSSH is not pinned, so it is skipped
/// there, by name.
fn ssh_keygen_or_skip(test: &str) -> bool {
    if cfg!(windows) {
        eprintln!("SKIPPED {test}: OpenSSH on Windows runners is not pinned");
        return false;
    }
    let out = Command::new("ssh-keygen").arg("-Y").output();
    let usage = out
        .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
        .unwrap_or_default();
    assert!(
        usage.contains("-Y"),
        "ssh-keygen with -Y (OpenSSH 8.0+) is required on this platform: {usage}"
    );
    true
}

/// A throwaway key, made by the test (never by Devplane), and a global git
/// configuration naming it.
struct Signer {
    dir: PathBuf,
    key: PathBuf,
    global: PathBuf,
    home: PathBuf,
}

fn signer(tag: &str) -> Signer {
    let dir = scratch(&format!("attest-{tag}")).canonicalize().unwrap();
    let key = dir.join("k");
    let out = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "throwaway", "-f"])
        .arg(&key)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let global = dir.join("gitconfig");
    std::fs::write(
        &global,
        format!(
            "[user]\n\tsigningkey = {}\n\temail = you@example.com\n",
            key.display()
        ),
    )
    .unwrap();
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    Signer {
        dir,
        key,
        global,
        home,
    }
}

/// A Devplane home whose store holds one finished change, its `check` run on
/// a real repository's clean tree. Returns the home and the change id.
async fn home_with_a_change(s: &Signer) -> (PathBuf, PathBuf, String) {
    let repo = s.dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("devplane.toml"), "[gates]\ncheck = [\"true\"]\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "init"]);
    let commit = git(&repo, &["rev-parse", "HEAD"]);
    let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
    let st = CommitStamp {
        commit: Some(commit),
        tree: Some(tree),
        tree_unknown: None,
        branch: Some("main".into()),
        clean: true,
        changed_files: 0,
        reach: Reach::NoRemote,
        remote: None,
    };
    let mut w = Change::new(
        devplane::core::ids::ProjectId::new("p1"),
        "Rate-limit the login route".into(),
        "do it".into(),
    );
    w.worktree = Some(repo.clone());
    w.gates.push(report(
        vec![CommandResult::exited("true", 0)],
        Some(st.clone()),
    ));
    let d = declared(&w);
    w.completion = Some(Completion::of(&w, Declared::Checks(&d), Some(&st)));
    let home = s.dir.join("devplane-home");
    std::fs::create_dir_all(&home).unwrap();
    let store = devplane::store::Store::open(&home.join("devplane.db"))
        .await
        .unwrap();
    store.save_change(&w).await.unwrap();
    (home, repo, w.id.as_str().to_string())
}

/// `devplane` with nothing of the caller's environment that could reach the
/// real home, a host, or the network.
fn devplane(s: &Signer, home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &s.home)
        .env("DEVPLANE_HOME", home)
        .env("GIT_CONFIG_GLOBAL", &s.global)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("NO_COLOR", "1")
        // Anything that tried the network would go here and fail.
        .env("HTTP_PROXY", "http://127.0.0.1:9")
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9")
        .output()
        .unwrap()
}

/// The commands under *Check the signature*, as printed.
fn printed_recipe(stdout: &str) -> Vec<String> {
    let at = stdout
        .find("## Check the signature")
        .expect("no recipe printed");
    let rest = &stdout[at..];
    let open = rest.find("```").unwrap();
    let body = &rest[open..];
    let first_nl = body.find('\n').unwrap();
    let close = body[first_nl..].find("\n```").unwrap() + first_nl;
    body[first_nl + 1..close]
        .lines()
        .map(str::to_string)
        .collect()
}

/// Runs the printed commands from a plain shell: no Devplane on `PATH`, no
/// environment beyond it.
fn plain_shell(dir: &Path, commands: &[String]) -> Output {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(commands.join(" && "))
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap()
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(files_under(&p));
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn a_signed_statement_verifies_with_ssh_keygen_alone() {
    if !ssh_keygen_or_skip("a_signed_statement_verifies_with_ssh_keygen_alone") {
        return;
    }
    let s = signer("verify");
    let (home, repo, id) = home_with_a_change(&s).await;
    let out_dir = s.dir.join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let f = out_dir.join("cert.intoto.json");
    let before = files_under(&s.home);

    let o = devplane(
        &s,
        &home,
        &repo,
        &[
            "change",
            "export",
            &id,
            "--sign",
            "ssh",
            "--out",
            f.to_str().unwrap(),
        ],
    );
    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(
        o.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );

    // Two files, nothing else; no key made anywhere under HOME.
    assert_eq!(
        files_under(&out_dir),
        vec![f.clone(), out_dir.join("cert.intoto.json.sig")]
    );
    assert_eq!(files_under(&s.home), before, "a file appeared under HOME");

    // The statement validates as in-toto v1 and says it is unsigned bytes.
    let st: Value = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap();
    assert_valid(&st);
    assert_eq!(st["predicate"]["result"], "PASSED");
    assert_eq!(st["predicate"]["devplane"]["signed"], false);
    assert_eq!(st["predicate"]["configuration"][1]["name"], "devplane.toml");

    let recipe = printed_recipe(&stdout);
    assert_eq!(recipe.len(), 2, "{stdout}");
    assert!(recipe[0].contains("you@example.com namespaces=\"certificate@devplane.dev\""));
    assert!(recipe[1].starts_with("ssh-keygen -Y verify"));
    assert!(stdout.contains("obtained yourself"), "{stdout}");
    let pubkey = std::fs::read_to_string(s.key.with_extension("pub")).unwrap();
    let body: Vec<&str> = pubkey.split_whitespace().take(2).collect();
    assert!(
        recipe[0].contains(&body.join(" ")),
        "not the global key: {}",
        recipe[0]
    );

    let ok = plain_shell(&out_dir, &recipe);
    assert!(
        ok.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&ok.stdout),
        String::from_utf8_lossy(&ok.stderr)
    );

    // One changed byte fails it.
    let mut bytes = std::fs::read(&f).unwrap();
    let at = bytes.iter().position(|b| *b == b'P').unwrap();
    bytes[at] = b'Q';
    std::fs::write(&f, &bytes).unwrap();
    let bad = plain_shell(&out_dir, &recipe);
    assert!(!bad.status.success(), "a changed statement still verified");

    // The same signature is not a git commit signature: the namespace differs.
    let sig = std::fs::read_to_string(out_dir.join("cert.intoto.json.sig")).unwrap();
    assert!(sig.starts_with("-----BEGIN SSH SIGNATURE-----"));
}

#[tokio::test]
async fn a_repository_local_signing_key_is_never_used() {
    if !ssh_keygen_or_skip("a_repository_local_signing_key_is_never_used") {
        return;
    }
    let s = signer("local");
    let (home, repo, id) = home_with_a_change(&s).await;
    // The agent's key, planted where an agent can write: the repository.
    let agent_key = s.dir.join("agent");
    Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&agent_key)
        .output()
        .unwrap();
    git(
        &repo,
        &["config", "user.signingkey", agent_key.to_str().unwrap()],
    );
    git(&repo, &["config", "gpg.ssh.program", "/bin/false"]);
    git(&repo, &["config", "gpg.format", "ssh"]);

    let f = s.dir.join("cert.intoto.json");
    let o = devplane(
        &s,
        &home,
        &repo,
        &[
            "change",
            "export",
            &id,
            "--sign",
            "ssh",
            "--out",
            f.to_str().unwrap(),
        ],
    );
    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(
        o.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let recipe = printed_recipe(&stdout);
    let global = std::fs::read_to_string(s.key.with_extension("pub")).unwrap();
    let planted = std::fs::read_to_string(agent_key.with_extension("pub")).unwrap();
    let g = global.split_whitespace().nth(1).unwrap();
    let p = planted.split_whitespace().nth(1).unwrap();
    assert!(
        recipe[0].contains(g) && !recipe[0].contains(p),
        "{}",
        recipe[0]
    );
    assert!(plain_shell(&s.dir, &recipe).status.success());
}

#[tokio::test]
async fn signing_refuses_and_writes_nothing() {
    if !ssh_keygen_or_skip("signing_refuses_and_writes_nothing") {
        return;
    }
    let s = signer("refuse");
    let (home, repo, id) = home_with_a_change(&s).await;
    let f = s.dir.join("out.json");
    let sig = s.dir.join("out.json.sig");
    let export = |s: &Signer| {
        devplane(
            s,
            &home,
            &repo,
            &[
                "change",
                "export",
                &id,
                "--sign",
                "ssh",
                "--out",
                f.to_str().unwrap(),
            ],
        )
    };
    let refused = |o: &Output, says: &str| {
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        assert!(!o.status.success(), "it signed: {err}");
        assert!(err.contains(says), "{err}");
        assert!(!f.exists() && !sig.exists(), "something was written");
    };

    // No key in the person's global configuration.
    std::fs::write(&s.global, "[user]\n\temail = you@example.com\n").unwrap();
    let o = export(&s);
    refused(&o, "no SSH signing key");
    assert!(String::from_utf8_lossy(&o.stderr).contains("Devplane never makes a key"));

    // A key that is not there.
    std::fs::write(&s.global, "[user]\n\tsigningkey = /nonexistent/key\n").unwrap();
    refused(&export(&s), "does not exist");

    // A key whose private half nobody holds: a literal with no agent.
    let public = std::fs::read_to_string(s.key.with_extension("pub")).unwrap();
    std::fs::write(
        &s.global,
        format!("[user]\n\tsigningkey = key::{}\n", public.trim()),
    )
    .unwrap();
    refused(&export(&s), "ssh-keygen could not sign");

    // No `ssh-keygen` on PATH (git still there).
    std::fs::write(
        &s.global,
        format!("[user]\n\tsigningkey = {}\n", s.key.display()),
    )
    .unwrap();
    let bin = s.dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let real_git = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let real_git = String::from_utf8_lossy(&real_git.stdout).trim().to_string();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_git, bin.join("git")).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args([
            "change",
            "export",
            &id,
            "--sign",
            "ssh",
            "--out",
            f.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env_clear()
        .env("PATH", &bin)
        .env("HOME", &s.home)
        .env("DEVPLANE_HOME", &home)
        .env("GIT_CONFIG_GLOBAL", &s.global)
        .output()
        .unwrap();
    refused(&o, "`ssh-keygen` was not found on PATH");

    // `--sign` without `--out` is a usage error.
    let o = devplane(
        &s,
        &home,
        &repo,
        &["change", "export", &id, "--sign", "ssh"],
    );
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );

    // An existing file is not replaced without `--force`.
    std::fs::write(&f, "mine").unwrap();
    let o = export(&s);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("pass --force"));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "mine");
    assert!(!sig.exists());
    let o = devplane(
        &s,
        &home,
        &repo,
        &[
            "change",
            "export",
            &id,
            "--sign",
            "ssh",
            "--force",
            "--out",
            f.to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(sig.exists());
}

#[tokio::test]
async fn an_unsigned_export_writes_the_same_statement_and_no_key_is_read() {
    let s = signer("unsigned");
    let (home, repo, id) = home_with_a_change(&s).await;
    // No signing key anywhere: an unsigned export never asks for one.
    std::fs::write(&s.global, "").unwrap();
    let a = devplane(&s, &home, &repo, &["--json", "change", "export", &id]);
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    let b = devplane(&s, &home, &repo, &["--json", "change", "export", &id]);
    assert_eq!(a.stdout, b.stdout, "two exports differ");
    let st: Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_valid(&st);
    let f = s.dir.join("st.json");
    let o = devplane(
        &s,
        &home,
        &repo,
        &[
            "--json",
            "change",
            "export",
            &id,
            "--out",
            f.to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        std::fs::read(&f).unwrap(),
        a.stdout,
        "the file is not what was printed"
    );
    assert!(!s.dir.join("st.json.sig").exists());
}

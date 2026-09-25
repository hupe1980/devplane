//! Two principles held over the record's shape: no stored metric about the person
//! who answered, and `verified` is computed from gate exits, never stored. Each
//! guard reads the SQL and structs directly and carries a planted name.

use std::path::Path;

/// Every column name in `schema.sql`, table by table.
fn columns() -> Vec<(String, String)> {
    let sql = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/schema.sql"))
        .expect("schema.sql");
    let mut out = Vec::new();
    let mut table: Option<String> = None;
    for raw in sql.lines() {
        let line = raw.split("--").next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("CREATE TABLE IF NOT EXISTS ") {
            table = rest.split(['(', ' ']).next().map(str::to_string);
            continue;
        }
        if line.starts_with(')') {
            table = None;
            continue;
        }
        if let Some(t) = &table
            && let Some(name) = line.split_whitespace().next()
            && name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            && !matches!(name, "primary" | "unique" | "foreign" | "check")
        {
            out.push((t.clone(), name.to_string()));
        }
    }
    out
}

/// Every `pub <name>:` field in a struct under `src/core/`, with its file.
fn core_fields() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).expect("src/core") {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/core")),
        &mut files,
    );
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for line in text.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("pub ")
                && let Some((name, _)) = rest.split_once(':')
                && !rest.starts_with("fn ")
                && !rest.starts_with("const ")
                && !rest.starts_with("static ")
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
            {
                out.push((f.display().to_string(), name.to_string()));
            }
        }
    }
    out
}

/// A name that measures the person rather than the tool or the question.
fn measures_the_person(name: &str) -> bool {
    const TELLS: &[&str] = &[
        "response_time",
        "answer_time",
        "time_to_answer",
        "answered_in",
        "answer_rate",
        "response_rate",
        "streak",
        "longest_wait",
        "review_time",
        "reviewed_for",
        "time_spent",
        "attention_score",
        "reviewer_speed",
        "review_duration",
        "time_to_decision",
        "reviewed_in",
    ];
    TELLS.iter().any(|t| name.contains(t))
}

#[test]
fn the_record_measures_the_question_and_never_the_person() {
    let cols = columns();
    let fields = core_fields();
    assert!(
        cols.len() > 30,
        "the schema reader found {} columns",
        cols.len()
    );
    assert!(
        fields.len() > 100,
        "the field reader found {} fields",
        fields.len()
    );
    // The guard notices what it is for.
    assert!(measures_the_person("answer_rate"));
    assert!(measures_the_person("longest_wait_secs"));
    assert!(!measures_the_person("asked_at"));

    let offending: Vec<String> = cols
        .iter()
        .map(|(t, c)| format!("{t}.{c}"))
        .chain(fields.iter().map(|(f, n)| format!("{f}: {n}")))
        .filter(|name| measures_the_person(name))
        .collect();
    assert!(
        offending.is_empty(),
        "a measurement of the person is being kept: {offending:?}"
    );
}

#[test]
fn verified_is_never_a_stored_value() {
    let cols = columns();
    assert!(!cols.is_empty());
    assert!(
        !cols.iter().any(|(_, c)| c == "verified"),
        "a column called verified would be a flag somebody clears"
    );
    let fields = core_fields();
    assert!(!fields.is_empty());
    let stored: Vec<_> = fields.iter().filter(|(_, n)| n == "verified").collect();
    assert!(
        stored.is_empty(),
        "verified is computed from gate exits against the tree, not carried as a field: {stored:?}"
    );
    // The sentence the principle uses is the one the certificate computes.
    let work = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/core/change.rs"))
        .unwrap();
    assert!(
        work.contains("pub fn of(change: &Change, project_declares_gates: bool"),
        "the completion is derived by a function of the work and the tree"
    );
}

/// The review measures the change, never its reader: nothing on the review path
/// names a review duration or reads a clock.
#[test]
fn the_review_times_nobody() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(measures_the_person("review_duration_secs"));
    assert!(measures_the_person("time_to_decision"));

    let mut sources: Vec<(String, String)> = vec![(
        "src/core/review.rs".into(),
        std::fs::read_to_string(root.join("src/core/review.rs")).expect("the review module"),
    )];
    let ui = root.join("ui/src/surfaces/review");
    for e in std::fs::read_dir(&ui)
        .expect("the review surface")
        .flatten()
    {
        let p = e.path();
        if p.is_file() {
            sources.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).unwrap_or_default(),
            ));
        }
    }
    assert!(
        sources.len() >= 3,
        "the sweep found {} files",
        sources.len()
    );
    for (name, text) in &sources {
        for word in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            assert!(
                !measures_the_person(&word.to_lowercase()),
                "{name} names `{word}`, a measurement of the person reviewing"
            );
        }
        for clock in ["Date.now()", "performance.now()", "new Date("] {
            assert!(!text.contains(clock), "{name} reads a clock: `{clock}`");
        }
    }
}

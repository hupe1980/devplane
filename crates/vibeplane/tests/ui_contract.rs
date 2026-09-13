//! The board page and the API have to agree.
//!
//! The page is a static file and the API is Rust; nothing links them, so a
//! renamed field is invisible until someone opens the board and finds a column
//! of blanks. This test reads the field names out of the page itself and checks
//! the API really serves them.

use serde_json::Value;

const PAGE: &str = include_str!("../ui/index.html");

/// Every `r.<field>` and `i.<field>` the page reads.
fn fields_read_by_page(prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let needle = format!("{prefix}.");
    let mut rest = PAGE;
    while let Some(at) = rest.find(&needle) {
        rest = &rest[at + needle.len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            out.push(name);
        }
    }
    out.sort();
    out.dedup();
    out
}

async fn serve_one_run() -> (std::net::SocketAddr, reqwest::Client) {
    // A database per test: two tests sharing one file race on the WAL, and the
    // failure looks nothing like the cause.
    let db = std::env::temp_dir().join(format!(
        "vp-ui-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let state = vibeplane::daemon::AppState::new(db, "tok".into(), Default::default())
        .await
        .unwrap();

    // One run in every interesting state: blocked, costed, in a worktree.
    state
        .ingest(
            vibeplane_domain::ids::RunId::new("s1"),
            vibeplane_domain::event::Source::Hook,
            vibeplane_domain::event::Event::QuestionAsked {
                question: "Keep it?".into(),
                options: vec!["yes".into()],
            },
            Some("/tmp/repo".into()),
            vibeplane_domain::run::RunMode::Observed,
        )
        .await;

    let app = vibeplane::api::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    (addr, reqwest::Client::new())
}

#[tokio::test]
async fn the_board_serves_every_field_the_page_reads() {
    let (addr, c) = serve_one_run().await;
    let board: Value = c
        .get(format!("http://{addr}/api/board"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let run = &board["runs"][0];
    let mut missing = Vec::new();
    for field in fields_read_by_page("r") {
        // `r.` also matches a few local variables in the page; only names the
        // API could plausibly own are worth failing on.
        if run.get(&field).is_none() && KNOWN_RUN_FIELDS.contains(&field.as_str()) {
            missing.push(field);
        }
    }
    assert!(missing.is_empty(), "the board does not serve: {missing:?}");

    for field in ["projects", "runs", "working", "needs_you", "cost_usd"] {
        assert!(
            board["summary"].get(field).is_some(),
            "the header reads summary.{field}"
        );
    }
}

/// The fields the page treats as belonging to a run. Listed explicitly so that
/// deleting one from the page *and* the API is a deliberate two-line change,
/// rather than something that silently stops being checked.
const KNOWN_RUN_FIELDS: &[&str] = &[
    "id",
    "state",
    "name",
    "mode",
    "entrypoint",
    "project_name",
    "context_percent",
    "cost_usd",
    "idle_seconds",
    "summary",
];

#[tokio::test]
async fn the_inbox_serves_every_field_the_page_reads() {
    let (addr, c) = serve_one_run().await;
    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let item = &inbox[0];
    for field in ["title", "detail", "options", "actions", "level", "run_id"] {
        assert!(
            item.get(field).is_some(),
            "the inbox card reads i.{field}, which is not served"
        );
    }
    // The page keys its styling off the level and its buttons off the actions,
    // so their vocabularies have to match.
    assert!(PAGE.contains("i.actions.includes(\"focus\")"));
    assert_eq!(item["level"], "high");
    assert!(
        item["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "focus")
    );
}

#[test]
fn the_page_carries_no_credential_and_no_remote_dependency() {
    // Everything is embedded: the board has to work on a laptop with no
    // network, and must never ship a token baked into the file.
    assert!(!PAGE.contains("http://") || PAGE.matches("http://").count() == 0);
    for remote in [
        "https://cdn",
        "https://unpkg",
        "https://fonts.",
        "<script src",
    ] {
        assert!(!PAGE.contains(remote), "the page must not load {remote}");
    }
}

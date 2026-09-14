//! The board page and the API have to agree.
//!
//! The page is a static file and the API is Rust; nothing links them, so a
//! renamed field is invisible until someone opens the board and finds a column
//! of blanks. This test reads the field names out of the page itself and checks
//! the API really serves them.

use serde_json::Value;

const PAGE: &str = include_str!("../ui/index.html");

/// Every `r.<field>`, `i.<field>` and `w.<field>` the page reads.
///
/// The variable name has to be the *whole* token: searching for `w.` alone also
/// finds the `w.` at the end of `window.`, and searching for `r.` finds one in
/// any identifier ending in `r`. That is why this used to need an allowlist to
/// stay quiet — an allowlist which then silently stopped covering every field
/// nobody had added to it.
fn fields_read_by_page(prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let needle = format!("{prefix}.");
    let bytes = PAGE.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = PAGE[from..].find(&needle) {
        let at = from + rel;
        from = at + needle.len();
        let preceded_by_word = at
            .checked_sub(1)
            .is_some_and(|i| bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_');
        if preceded_by_word {
            continue;
        }
        let name: String = PAGE[from..]
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
    let state = vibeplane::daemon::AppState::new(
        db.clone(),
        "tok".into(),
        Default::default(),
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .unwrap();

    // One run in every interesting state: blocked, costed, in a worktree.
    state
        .ingest(
            vibeplane::core::ids::RunId::new("s1"),
            vibeplane::core::event::Source::Hook,
            vibeplane::core::event::Event::QuestionAsked {
                question: "Keep it?".into(),
                options: vec!["yes".into()],
            },
            Some("/tmp/repo".into()),
            vibeplane::core::run::RunMode::Observed,
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
        if run.get(&field).is_none() {
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

#[test]
fn every_button_the_inbox_renders_is_one_the_item_offered() {
    // `attention.rs` is careful about which actions an item gets: `attach` is
    // withheld from a driven run because there is no terminal to attach to,
    // `allow` from a session Vibeplane only watches because it cannot answer
    // for it. All of that is wasted if the page renders the button anyway —
    // and for `attach` and `snooze` it did, unconditionally.
    //
    // So: every action button in the inbox card must be behind a check that
    // the item listed it. This reads the page rather than trusting a reviewer
    // to notice the next one.
    let card = PAGE
        .split("$(\"inbox\").innerHTML")
        .nth(1)
        .expect("the inbox is rendered")
        .split("$(\"worksec\")")
        .next()
        .expect("the inbox block ends");

    let mut ungated = Vec::new();
    for (at, _) in card.match_indices("data-act=\"") {
        let action: String = card[at + 10..].chars().take_while(|c| *c != '"').collect();
        // The guard sits on the same template line as the button.
        let line_start = card[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = &card[line_start..at];
        if !line.contains(&format!("i.actions.includes(\"{action}\")")) {
            ungated.push(action);
        }
    }
    assert!(
        ungated.is_empty(),
        "these inbox buttons render without checking the item offers them: {ungated:?}. \
         An action the daemon withheld is one the surface cannot perform."
    );
}

#[test]
fn work_items_can_be_snoozed_through_a_route_that_exists() {
    // The items only Work produces outlive the sessions that made them, so
    // they frequently carry no run id at all. Sending their `snooze` to
    // `/api/runs//snooze` reached nothing.
    assert!(
        PAGE.contains("`/api/work/${req}`"),
        "the page must snooze work through the work route"
    );
    assert!(
        PAGE.contains("`/api/runs/${run}`"),
        "and a run through the run route"
    );
    // And the button has to carry the work id, or the choice above has nothing
    // to make itself on.
    // `run_id` is optional on the wire for exactly this reason, so the button
    // renders an empty string rather than the word `null` when there is no run.
    assert!(
        PAGE.contains(
            r#"data-act="snooze" data-run="${esc(i.run_id || "")}" data-work="${esc(i.work_id || "")}""#
        ),
        "the snooze button must carry the work id, and tolerate having no run"
    );
}

#[test]
fn every_element_the_script_reaches_for_exists_in_the_page() {
    // `$("q")` is `getElementById`, and a typo returns `null`. The page wires
    // its listeners at load, so one wrong id is a `TypeError` before anything
    // renders — a blank board, with the cause only visible in a console nobody
    // has open. Nothing else in this repository would catch it: the page is a
    // static file that no compiler reads.
    let ids: std::collections::HashSet<&str> = PAGE
        .match_indices("id=\"")
        .map(|(at, _)| {
            let rest = &PAGE[at + 4..];
            &rest[..rest.find('"').unwrap_or(0)]
        })
        .collect();

    let mut missing = Vec::new();
    for (at, _) in PAGE.match_indices("$(\"") {
        let rest = &PAGE[at + 3..];
        let name = &rest[..rest.find('"').unwrap_or(0)];
        if !name.is_empty() && !ids.contains(name) {
            missing.push(name);
        }
    }
    missing.sort_unstable();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "the script looks up elements the page does not define: {missing:?}"
    );
}

#[test]
fn the_board_can_search_what_the_cli_can() {
    // The daemon has had the index all along — `/api/search` is what
    // `vibeplane search` uses — and the board was the one surface that could
    // not answer "where did I see that command".
    assert!(
        PAGE.contains("/api/search?q="),
        "the board must be able to search"
    );
    // Debounced and sequenced: a query per keystroke is a query per keystroke,
    // and a slow early answer must not overwrite a fast later one.
    assert!(
        PAGE.contains("searchSeq") && PAGE.contains("setTimeout"),
        "search must be debounced and its results sequenced"
    );
    // The query is a phrase a person typed, so it has to be encoded.
    assert!(
        PAGE.contains("encodeURIComponent"),
        "the query must be encoded, not interpolated"
    );
}

#[test]
fn the_answers_an_agent_offered_are_answerable() {
    // The gap this closes: the daemon carried the agent's options with their
    // protocol ids, the API accepted `option_id`, two documents promised
    // `1`-`9` picks one — and the page rendered them as an unclickable list
    // while offering `allow`/`deny`. Every layer had its half and none of them
    // connected, so the one question Vibeplane could truly answer became a
    // yes/no nobody had asked.
    assert!(
        PAGE.contains(r#"data-act="choose""#),
        "an option the agent offered must be something you can pick"
    );
    assert!(
        PAGE.contains("data-opt=\"${esc(o.id)}\""),
        "and picking it must send that option's own id, not a guessed string"
    );
    assert!(
        PAGE.contains("option_id: opt"),
        "the choice goes to the API as option_id"
    );
    // An option with no id belongs to a provider dialog Vibeplane cannot
    // answer. Readable, never dressed up as a button.
    assert!(
        PAGE.contains("opt-dead"),
        "an unanswerable option is shown but not offered"
    );
    // `1`-`9`, finally wired.
    assert!(
        PAGE.contains(r#"e.key >= "1" && e.key <= "9""#),
        "the number keys pick an answer"
    );
    // `r` was refresh on a page that is already live over SSE.
    assert!(
        !PAGE.contains(r#"e.key === "r") refresh()"#),
        "r is reply now; refresh happens by itself"
    );
    assert!(PAGE.contains(r#"r: "reply""#), "r replies");
}

#[test]
fn the_launchers_are_the_only_things_that_cover_the_page() {
    // The design rule, enforced rather than written down: a decision is made
    // while looking at its evidence, so nothing that asks for one may cover
    // the page. Only `⌘K` and `⌘N` may, because neither is a decision.
    for id in ["palette", "dispatch", "why"] {
        assert!(
            PAGE.contains(&format!(r#"id="{id}""#)),
            "the page must have a {id} surface"
        );
    }
    assert!(
        PAGE.contains(r#"e.key === "k""#) && PAGE.contains(r#"e.key === "n""#),
        "the two launchers need their shortcuts"
    );
    // Escape always gets out of one, from anywhere.
    assert!(
        PAGE.contains("if (e.key === \"Escape\") { e.preventDefault(); return closeOver(); }"),
        "escape must leave an overlay"
    );
}

#[test]
fn dispatch_refuses_a_project_nobody_trusted() {
    // A headless agent runs the repository's own hooks and MCP servers with no
    // dialog of its own, so the trust gesture is deliberate and per directory.
    // The launcher must say so rather than letting the daemon refuse after the
    // fact — a button that fails is worse than one that explains.
    assert!(
        PAGE.contains("is not trusted"),
        "the launcher explains an untrusted project"
    );
    assert!(
        PAGE.contains("$(\"dgo\").disabled = !p || !p.trusted"),
        "and does not offer to start one"
    );
    // And it says what will happen before it happens.
    assert!(PAGE.contains("dwill"), "the launcher previews the effect");
}

#[test]
fn the_why_pane_reads_the_decision_log_and_nothing_else() {
    // `?` answers "why is this here" from the same rows `vibeplane audit`
    // prints. Inventing an explanation would be the one thing a supervision
    // tool cannot do.
    assert!(
        PAGE.contains("/api/decisions?limit=25&about="),
        "the why pane asks the decision log"
    );
    assert!(
        PAGE.contains("Nothing has been decided about this yet"),
        "and says so plainly when there is nothing, rather than guessing"
    );
}

#[test]
fn the_board_loads_the_projects_a_person_can_dispatch_to() {
    assert!(
        PAGE.contains(r#"api("/api/projects")"#),
        "the launcher needs every project, not only the ones with a session"
    );
}

/// The work card reads fields too, and nothing was checking them.
///
/// The run rows and the inbox each had this test; the work rows did not, which
/// is why `stopped_summary` could be added to the page and the API and agree by
/// luck. A field the page reads and the API does not serve is a blank column,
/// and a blank column on a supervision surface reads as "nothing to report" —
/// the opposite of what is true.
#[test]
fn the_work_card_reads_only_fields_the_api_serves() {
    // The row itself is a flattened `Work`, so its own field names are the
    // domain's and a rename breaks here.
    let work = vibeplane::core::Work::new(
        vibeplane::core::ProjectId::from_path(std::path::Path::new("/tmp/x")),
        vibeplane::core::WorkKind::Quick,
        "t".into(),
        "p".into(),
    );
    let row = serde_json::to_value(&work).expect("a Work serialises");

    // What `WorkView` adds on top, listed explicitly so that removing one is a
    // deliberate two-line change rather than something that stops being checked.
    const VIEW_ONLY: &[&str] = &["gate", "can_retry", "stopped_summary"];

    let mut missing = Vec::new();
    for field in fields_read_by_page("w") {
        if row.get(&field).is_none() && !VIEW_ONLY.contains(&field.as_str()) {
            missing.push(field);
        }
    }
    assert!(
        missing.is_empty(),
        "the work card reads fields the API does not serve: {missing:?}"
    );
}

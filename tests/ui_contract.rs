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
    let state = devplane::daemon::AppState::new(
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
            devplane::core::ids::RunId::new("s1"),
            devplane::core::event::Source::Hook,
            devplane::core::event::Event::QuestionAsked {
                question: "Keep it?".into(),
                options: vec!["yes".into()],
            },
            Some("/tmp/repo".into()),
            devplane::core::run::RunMode::Observed,
        )
        .await;

    let app = devplane::api::router(state);
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
    // `allow` from a session Devplane only watches because it cannot answer
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
    // `devplane search` uses — and the board was the one surface that could
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
    // connected, so the one question Devplane could truly answer became a
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
    // An option with no id belongs to a provider dialog Devplane cannot
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
    // `?` answers "why is this here" from the same rows `devplane audit`
    // prints. Inventing an explanation would be the one thing a supervision
    // tool cannot do.
    //
    // **This property moved on 2026-09-18 and caught itself moving.** It used
    // to assert both halves against the page, and the pane is now rendered by
    // the daemon — so the page-side half went on passing while the sentence it
    // guarded had left the file. That is the exact failure the phase widening
    // the escaping property exists to prevent, and it happened here first.
    assert!(
        PAGE.contains("/api/decisions/pane?"),
        "the why pane asks the decision log"
    );
    let render = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render.rs"),
    )
    .expect("src/render.rs");
    assert!(
        render.contains("Nothing has been decided about this yet"),
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
    let work = devplane::core::Work::new(
        devplane::core::ProjectId::from_path(std::path::Path::new("/tmp/x")),
        devplane::core::WorkKind::Quick,
        "t".into(),
        "p".into(),
    );
    let row = serde_json::to_value(&work).expect("a Work serialises");

    // What `WorkView` adds on top, listed explicitly so that removing one is a
    // deliberate two-line change rather than something that stops being checked.
    const VIEW_ONLY: &[&str] = &[
        "gate",
        "can_retry",
        "stopped_summary",
        "claim",
        // Why there is no claim, where there could have been one: a repository
        // that keeps no transcripts and an agent that said nothing are two
        // facts, and the page renders a different sentence for each.
        "claim_absent",
    ];

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

/// The board's script has to *parse*.
///
/// It shipped not parsing. Two `const hit` in one block scope is a
/// `SyntaxError`, which takes the whole `<script>` with it — so `devplane
/// open` served a page that rendered its chrome, said "connecting", and never
/// fetched anything. Every other test here reads the page as *text*: they
/// check that the fields the page names match the ones the API serves, which
/// stays true of a file that cannot run at all.
///
/// Shelling out to `node --check` rather than linking a JavaScript parser: the
/// page is 900 lines of plain script, the check is the one a browser does, and
/// a parser crate would be a dependency bought for a single assertion. Skipped
/// with a printed note where `node` is absent, and CI has it.
#[test]
fn the_board_script_parses() {
    let script = PAGE
        .split_once("<script>")
        .and_then(|(_, rest)| rest.split_once("</script>"))
        .map(|(body, _)| body)
        .expect("the board has one inline <script>");
    assert!(
        script.len() > 10_000,
        "extracted {} bytes — the script tags moved",
        script.len()
    );

    let dir = std::env::temp_dir().join("devplane-ui-contract");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let js = dir.join("board.js");
    std::fs::write(&js, script).expect("write the extracted script");

    match std::process::Command::new("node")
        .arg("--check")
        .arg(&js)
        .output()
    {
        Ok(out) if out.status.success() => {}
        Ok(out) => panic!(
            "the board's script does not parse, so the whole page is inert:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(_) => eprintln!("node is not installed; the board's script was not parsed"),
    }
    std::fs::remove_file(&js).ok();
}

/// A value read straight off the API is escaped, every time, without anybody
/// deciding whether this particular one needs it.
///
/// The page renders agent output, issue titles, branch names and error text
/// written by people who are not the reader. Safety rested on somebody
/// remembering `esc()` at a hundred and forty-nine interpolation sites, and
/// while the discipline held — nothing unescaped was ever found to be
/// exploitable — "we have been careful so far" is not a security property.
///
/// The rule is deliberately uniform rather than clever: an interpolation that
/// is a bare property path (`r.state`, `s.projects`, `w.pull_request.number`)
/// is escaped whether it holds a string or a number. Escaping a number costs
/// nothing; deciding case by case is how one gets missed. Expressions that do
/// arithmetic, compare, or call something are judged on their own and are not
/// what this catches.
#[test]
fn every_value_read_off_the_api_is_escaped() {
    let mut bare = Vec::new();
    for (at, _) in PAGE.match_indices("${") {
        let rest = &PAGE[at + 2..];
        let Some(end) = rest.find('}') else { continue };
        let expr = rest[..end].trim();
        // A bare dotted path and nothing else: no call, no operator, no ternary.
        let is_path = expr.split('.').all(|seg| {
            !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }) && expr.contains('.')
            && expr.starts_with(|c: char| c.is_ascii_lowercase());
        // `state.sel` addresses a CSS selector rather than markup.
        if is_path && expr != "state.sel" {
            bare.push(expr.to_string());
        }
    }
    assert!(
        bare.is_empty(),
        "these values reach the page unescaped: {bare:?}. \
         Wrap each in esc() — a number costs nothing to escape."
    );
}

/// The page announces, and announces one thing.
///
/// Twenty sessions changing state must not become twenty announcements, so
/// there is exactly one region and it announces the count that decides whether
/// to look — never the thing that changed.
#[test]
fn there_is_exactly_one_live_region_and_it_is_polite() {
    assert_eq!(
        PAGE.matches("aria-live").count(),
        1,
        "one live region: a second one is a second thing talking over the first"
    );
    assert!(PAGE.contains(r#"aria-live="polite""#), "never assertive");
    assert!(
        PAGE.contains(r#"id="announce""#) && PAGE.contains(r#"aria-atomic="true""#),
        "the whole sentence is read, not the character that changed"
    );
    assert!(
        PAGE.contains(r#"$("announce").textContent !== say"#),
        "it must only speak when the sentence changes, or every poll is an announcement"
    );
}

/// Anything that covers the page says so, and gives the keyboard back.
#[test]
fn every_overlay_is_a_dialog_and_returns_focus() {
    // The overlays are *found*, never listed. A hardcoded list is an allowlist,
    // and an allowlist stops covering the thing somebody adds next — which is
    // the failure this file already learned once, on the field scanner above.
    // Anything that covers the page carries `class="over"`; the transcript is
    // the one panel that covers it without being a launcher.
    let mut overlays: Vec<&str> = PAGE
        .match_indices(r#"class="over""#)
        .map(|(at, _)| {
            let tag_start = PAGE[..at].rfind('<').expect("an opening tag");
            let tag_end = PAGE[tag_start..]
                .find('>')
                .map_or(PAGE.len(), |i| tag_start + i);
            &PAGE[tag_start..tag_end]
        })
        .collect();
    overlays.push({
        let at = PAGE.find(r#"id="talk""#).expect("the transcript exists");
        let tag_start = PAGE[..at].rfind('<').expect("an opening tag");
        let tag_end = PAGE[tag_start..]
            .find('>')
            .map_or(PAGE.len(), |i| tag_start + i);
        &PAGE[tag_start..tag_end]
    });
    assert!(
        overlays.len() >= 5,
        "only {} overlays found; the scan is broken, not the page",
        overlays.len()
    );
    for tag in overlays {
        let id = tag
            .split(r#"id=""#)
            .nth(1)
            .and_then(|r| r.split('"').next())
            .unwrap_or("?");
        assert!(
            tag.contains(r#"role="dialog""#) && tag.contains(r#"aria-modal="true""#),
            "{id} covers the page and does not say so: {tag}"
        );
        assert!(
            tag.contains("aria-label="),
            "{id} has no name a screen reader can read: {tag}"
        );
    }
    // And closing puts the keyboard back where it was.
    assert!(
        PAGE.contains("over.cameFrom") && PAGE.contains("talk.cameFrom"),
        "a dialog that drops focus on the body strands a keyboard user at the top of the page"
    );
}

/// Every state that shows as a glyph also reads as a word.
///
/// "State is never carried by colour alone. Every state has a glyph and a
/// word" — but the board row carried the glyph and nothing else, so a screen
/// reader announced a run's state as "●".
#[test]
fn a_glyph_that_carries_state_has_a_word_beside_it() {
    assert!(
        PAGE.contains(".sr {"),
        "there has to be a way to say something to a screen reader and nobody else"
    );
    for (at, _) in PAGE.match_indices("GLYPH[") {
        let line_start = PAGE[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = PAGE[at..].find('\n').map(|i| at + i).unwrap_or(PAGE.len());
        let line = &PAGE[line_start..line_end];
        // The fallback table for a missing summary is prose, not a glyph.
        if !line.contains("<span") {
            continue;
        }
        assert!(
            line.contains(r#"class="sr""#) && line.contains(r#"aria-hidden="true""#),
            "a glyph carrying state with no word beside it: {}",
            line.trim()
        );
    }
}

/// The board's list sections are lists.
#[test]
fn the_sections_that_hold_rows_say_they_are_lists() {
    for id in ["inbox", "board", "work"] {
        let at = PAGE
            .find(&format!(r#"id="{id}""#))
            .unwrap_or_else(|| panic!("{id} exists"));
        let tag_end = PAGE[at..].find('>').map(|i| at + i).unwrap_or(PAGE.len());
        assert!(
            PAGE[at..tag_end].contains(r#"role="list""#),
            "{id} holds rows and does not present as a list"
        );
    }
    assert!(
        PAGE.matches(r#"role="listitem""#).count() >= 3,
        "the rows inside those lists are the items of them"
    );
}

/// The page renders, and nothing a stranger wrote comes back as markup.
///
/// Every other test here reads the page as *text*, so none can see a template
/// that throws or prove that a title carrying `<img onerror=...>` arrives
/// escaped. This runs the script against a stub DOM and checks what lands in
/// `innerHTML`; `tests/ui_render.js` holds the fixtures and the assertions.
///
/// Node is not a build dependency, so a machine without it skips. CI has it.
#[test]
fn the_page_renders_and_escapes_what_it_renders() {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ui_render.js");
    let page = concat!(env!("CARGO_MANIFEST_DIR"), "/ui/index.html");
    let out = match std::process::Command::new("node")
        .arg(script)
        .arg(page)
        .output()
    {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipped: node is not installed");
            return;
        }
        Err(e) => panic!("could not run node: {e}"),
    };
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
}

/// The page is one file, it asks the network for nothing, and it stays small.
///
/// The shell decision rests on these numbers, and a trigger nobody measures is
/// a trigger nobody reaches.
///
/// The external-request check is the load-bearing half: a CDN font or an
/// off-machine script tag breaks the board over Tailscale on a phone and
/// `curl`ing the page to debug it, silently, on someone else's network.
#[test]
fn the_page_is_one_small_self_contained_file() {
    let lines = PAGE.lines().count();

    // **Lines, not bytes, and the change is a correction rather than a
    // relaxation.**
    //
    // The ceiling here has always been a proxy for one question: *has this
    // become an application a bundler would help with?* Bytes answered it
    // badly. The page is **31 % comments**, and those comments are why the
    // escaping rules and the accessibility properties are still true — so a
    // byte ceiling made every line of explanation compete with every feature,
    // which is a trade nobody chose and the wrong one to force.
    //
    // Lines measure the thing directly, in the unit a person reads and
    // maintains. At 3 000 the question is genuinely worth re-opening.
    //
    // **The byte number was never about what a user pays.** If transfer ever
    // matters, the answer is a compression layer — measured at +9 crates, a
    // 69 % reduction — and not fewer comments.
    assert!(
        lines < 3_000,
        "the page is {lines} lines, past the point where a build step is worth revisiting"
    );

    // And the obvious way to evade a line ceiling is one very long line, so
    // that is closed here rather than discovered later. Nothing hand-written
    // reaches this; a minified blob does immediately.
    if let Some(long) = PAGE.lines().find(|l| l.chars().count() > 400) {
        panic!(
            "a line is {} characters: {}…\nA line nobody can read is a build step that arrived without being decided",
            long.chars().count(),
            long.chars().take(60).collect::<String>()
        );
    }

    // Anything that would make the browser fetch from somewhere else.
    for (at, _) in PAGE.match_indices("//") {
        let line = &PAGE[PAGE[..at].rfind('\n').map_or(0, |i| i + 1)..];
        let line = &line[..line.find('\n').unwrap_or(line.len())];
        let is_url = PAGE[..at].ends_with("http:") || PAGE[..at].ends_with("https:");
        if !is_url {
            continue;
        }
        let rest = &PAGE[at + 2..];
        let host: String = rest
            .chars()
            .take_while(|c| !"/\"' )".contains(*c))
            .collect();
        assert!(
            host.starts_with("127.0.0.1")
                || host.starts_with("localhost")
                || host.contains("w3.org"),
            "the page would reach {host}, and it must ask the network for nothing: {}",
            line.trim()
        );
    }
    for tag in [
        "<script src",
        "<link rel=\"stylesheet\"",
        "@import",
        "<iframe",
    ] {
        assert!(
            !PAGE.contains(tag),
            "`{tag}` loads a second file; the page is served as one"
        );
    }
}

/// The page says what language it is in.
///
/// A screen reader picks its voice and its pronunciation from `lang`, and with
/// no `<html>` element at all there was nothing to read it from. The page was
/// otherwise careful about accessibility — roles, a live region, `aria-current`,
/// visually-hidden text beside every glyph — which is what made the omission
/// easy to keep: everything a sighted reviewer checks was already there.
#[test]
fn the_page_declares_its_language() {
    assert!(
        PAGE.contains("<html lang="),
        "the board must declare a language for a screen reader to read it in"
    );
}

/// Tab cannot leave an open dialog.
///
/// `aria-modal="true"` is a promise that the rest of the page is inert. The
/// veil makes that true for a mouse and did nothing for a keyboard: Tab walked
/// out of the palette into the board behind it, where every row is focusable
/// and none of it is announced.
#[test]
fn a_modal_dialog_keeps_the_keyboard() {
    let modals = PAGE.matches("aria-modal=\"true\"").count();
    assert!(modals >= 4, "expected the page's dialogs, found {modals}");
    assert!(
        PAGE.contains("const trapTab"),
        "a dialog that claims aria-modal has to hold the keyboard"
    );
    assert!(
        PAGE.contains("trapTab(e)"),
        "the trap has to be wired into the keydown handler, not merely defined"
    );
}

/// Every cell a session row puts between the name and the summary has a width.
///
/// The design language says *"every number is tabular and right-aligned, so a
/// column of costs and a column of context percentages can be scanned rather
/// than read"* — and that is a property of the **column**, not of the cell.
/// `.gauge` and `.cost` were right-aligned with fixed widths while the cell in
/// front of them had none, so a session on `cli` instead of `claude-vscode`
/// shifted every number after it ten characters to the left. The rule was
/// stated, the cells obeyed it individually, and the column it exists for did
/// not survive one short word.
///
/// A property nobody can see in a screenshot of a machine where every session
/// happens to run the same surface needs a mechanism, not an intention.
#[test]
fn every_column_between_the_name_and_the_summary_is_sized() {
    let row = PAGE
        .split_once("card row ${esc(r.state)}")
        .expect("the session row template")
        .1;
    let row = &row[..row.find("</div>").expect("the row closes")];

    let mut unsized_cells = Vec::new();
    for class in ["name", "surface", "gauge", "cost", "since"] {
        assert!(
            row.contains(&format!("class=\"{class}\""))
                || row.contains(&format!("class=\"{class} ")),
            "the session row no longer has a `{class}` cell — if it was renamed, \
             rename it here too rather than deleting the check"
        );
        // A width for that class, anywhere in the stylesheet.
        let sized = PAGE
            .lines()
            .filter(|l| l.contains(&format!(".{class} ")) || l.contains(&format!(".{class},")))
            .any(|l| l.contains("width:"));
        if !sized {
            unsized_cells.push(class);
        }
    }
    assert!(
        unsized_cells.is_empty(),
        "these cells sit in a column a person is meant to scan and have no width: \
         {unsized_cells:?} — one short value in an earlier cell moves every number after it"
    );
}

/// The viewport tag has a breakpoint behind it.
///
/// The page has declared `width=device-width` since its first commit and had a
/// single media query — `prefers-color-scheme` — behind it. A viewport tag is a
/// promise that the layout responds to the width, and the fixed column widths
/// that make twenty sessions scannable on a monitor are exactly what makes one
/// row wider than a phone: measured at a 500 px viewport, the document scrolled
/// to 547.
///
/// This pins the *mechanism*, not the layout — nothing in this suite runs a
/// layout engine, and a test that claimed to check the rendered width would be
/// claiming more than it can see.
#[test]
fn the_page_has_a_breakpoint_behind_its_viewport_tag() {
    assert!(
        PAGE.contains("width=device-width"),
        "the page claims to respond to the viewport"
    );
    assert!(
        PAGE.contains("@media (max-width:"),
        "and has to have something behind that claim"
    );
}

/// Closing works on every overlay, including the one somebody adds next.
///
/// `openOver` and `closeOver` walk a list, and a dialog missing from it opens
/// and never closes — `esc` does nothing, the veil does nothing, and the page
/// is stuck. The list is small enough to keep correct by hand and exactly the
/// kind of thing nobody remembers, so it is checked against the page.
#[test]
fn every_overlay_is_in_the_list_that_closes_them() {
    let declared = PAGE
        .split("const OVERLAYS = [")
        .nth(1)
        .and_then(|r| r.split(']').next())
        .expect("the page declares its overlays");
    for (at, _) in PAGE.match_indices(r#"class="over""#) {
        let tag_start = PAGE[..at].rfind('<').expect("an opening tag");
        let id = PAGE[tag_start..at]
            .split(r#"id=""#)
            .nth(1)
            .and_then(|r| r.split('"').next())
            .expect("an overlay has an id");
        assert!(
            declared.contains(&format!("\"{id}\"")),
            "`{id}` covers the page and is not in OVERLAYS, so nothing closes it"
        );
    }
}

/// An empty board says *which* kind of empty it is.
///
/// "No sessions" and "no agents running on this machine" are different
/// sentences and only one of them is reassuring. The board used to show a
/// heading over blank space, which says neither — and it is the first screen
/// anybody sees.
#[test]
fn the_empty_board_has_two_sentences_and_only_one_is_a_thing_to_do() {
    let body = PAGE
        .split("function emptyBoard(")
        .nth(1)
        .and_then(|r| r.split("\nfunction ").next())
        .expect("the page has an empty-state function");
    // The one that is a thing to do names the command that does it.
    assert!(
        body.contains("devplane connect claude"),
        "the not-connected state has to name the command that fixes it"
    );
    // The reassuring one does not, and is reached when the answer is unknown:
    // telling somebody to run a command they have already run is worse than
    // saying nothing.
    assert!(
        body.contains("state.connected === false"),
        "only a definite `no` earns the sentence that asks for work"
    );
    assert!(
        body.contains("Nothing is running right now"),
        "a working machine with nothing on it needs the reassuring sentence"
    );
    // Quiet sessions are the usual reason a working machine looks empty, and a
    // count with nothing to click is a dead end.
    assert!(
        body.contains("class=\"linkish dormant\""),
        "the quiet count has to be the way to see them"
    );
}

/// The setup panel reads and never writes.
///
/// Not an unfinished form: the permission rules are committed files reviewed
/// like code, and an agent on this machine runs as the same user and can read
/// the token this page uses. A POST that edited `[policy]` would be the
/// widening path the gate exists to close. The panel says so, in the panel.
#[test]
fn the_setup_panel_reads_the_configuration_and_never_writes_it() {
    let body = PAGE
        .split(r#"<div id="setup" class="over""#)
        .nth(1)
        .and_then(|r| r.split("</div>\n\n").next())
        .expect("the page has a setup panel");
    assert!(
        body.contains("never writes them"),
        "a person who came looking for a settings form is owed the reason there is none"
    );
    // The only request it may make, and the method it may not.
    let script = PAGE
        .split("async function openSetup(")
        .nth(1)
        .and_then(|r| r.split("\nfunction ").next())
        .expect("the panel fetches something");
    assert!(script.contains(r#"api("/api/setup")"#));
    assert!(
        !script.contains("POST"),
        "nothing in this panel writes; the editing happens where the review does"
    );
}

/// Nothing in this product is reachable by keyboard alone.
///
/// The board is keyboard-first and was, for a while, keyboard-only: five
/// commands — the palette, dispatch, the forge list, the setup panel and the
/// reason key — had a shortcut and no target anywhere on the page. A shortcut
/// nobody can discover is a feature only its author has.
///
/// The footer carries the global ones, each as a button printing its own key,
/// so the legend and the toolbar are the same thing. This checks the wiring
/// rather than the list: a button added to the footer with no entry in `GO`
/// does nothing when pressed, and that is silent.
#[test]
fn every_global_command_has_something_to_click() {
    let footer = PAGE
        .split("<footer>")
        .nth(1)
        .and_then(|r| r.split("</footer>").next())
        .expect("the page has a footer");
    let table = PAGE
        .split("const GO = {")
        .nth(1)
        .and_then(|r| r.split("\n};").next())
        .expect("the page declares what its footer buttons do");

    let mut found = 0;
    for (at, _) in footer.match_indices(r#"data-go=""#) {
        let name = footer[at + 9..]
            .split('"')
            .next()
            .expect("a data-go carries a name");
        assert!(
            table.contains(&format!("{name}:")),
            "the footer offers `{name}` and nothing happens when it is pressed"
        );
        found += 1;
    }
    assert!(found >= 5, "only {found} global commands have a target");

    // And the two row-scoped ones that had no target either.
    assert!(
        PAGE.contains(r#"button[data-why]"#) && PAGE.contains(r#"data-why="${n}""#),
        "the decision log for a row was on the ? key and on nothing else"
    );
    assert!(
        PAGE.contains(r#"card.classList.contains("row") && card.dataset.run"#),
        "a session row has to open what it is saying, the way enter does"
    );
}

/// The work view shows what the gate measured, and says what it does not show.
///
/// Two properties, and the
/// second is the one that matters: command output is the compiler's bytes and
/// the agent's, so it is escaped like everything else the page prints — and
/// every way the evidence can be missing is a sentence rather than a blank
/// region, because an empty space where evidence belongs reads as broken.
#[test]
fn the_work_view_shows_the_gate_evidence_and_names_what_is_missing() {
    let body = PAGE
        .split("function renderWork(")
        .nth(1)
        .and_then(|r| r.split("\n}\n").next())
        .expect("the page renders a work view");

    // Every value from the gate is escaped. `command` and `output_tail` are the
    // two that carry somebody else's text.
    // The prefix, not the whole call: `esc(c.output_tail || "")` escapes the
    // value exactly as `esc(c.output_tail)` does, and a test that insists on one
    // spelling fails on code that is right — which it did, first time out.
    for field in ["c.command", "c.output_tail", "g.summary", "g.name"] {
        assert!(
            body.contains(&format!("esc({field}")),
            "`{field}` reaches the page unescaped"
        );
    }

    // Three ways a gate can say nothing, three sentences. A gate that never
    // ran, a gate that ran and recorded no commands, and a reproduction gate
    // whose red *is* its green — the last row of the spec's edge-case walk.
    assert!(
        body.contains("No gate has run against this work yet"),
        "a work nothing has checked must not read as a work that passed"
    );
    assert!(
        body.contains("recorded no commands, which is not the same as passing"),
        "an empty command list must not read as a pass"
    );
    assert!(
        body.contains("failing is the point"),
        "a reproduction gate has to explain its own verdict"
    );

    // The change is fetched and inserted, not computed here: a client-side diff
    // renderer is the change that would force a bundler onto this page.
    assert!(
        body.contains("loadChange(w.id)"),
        "the view has to ask for the change"
    );
    assert!(
        PAGE.contains("box().innerHTML = changes.html"),
        "and insert what the daemon rendered rather than building it"
    );

    // The release control is offered only where it can keep its promise.
    assert!(
        body.contains(r#"w.phase === "human""#) && body.contains(r#"data-act="approve""#),
        "a button that cannot do what it says is the one thing this must not show"
    );
}

/// The two ways an agent's account can be missing read differently.
///
/// `[transcripts] keep = false` means **nothing was recorded**; a kept
/// transcript with no closing message means **the agent said nothing**. Both
/// arrived as an absent `claim` until the API was made to say which, and a
/// surface that renders them identically tells a reviewer something false about
/// a repository that simply never writes transcripts down.
#[test]
fn the_two_ways_a_claim_can_be_absent_do_not_render_the_same() {
    let body = PAGE
        .split("function renderWork(")
        .nth(1)
        .and_then(|r| r.split("\n}\n").next())
        .expect("the page renders a work view");

    assert!(
        body.contains(r#"w.claim_absent === "transcripts_off""#),
        "the page has to ask which absence this is"
    );
    // Two sentences, and neither is the other.
    assert!(body.contains("Nothing was recorded"), "{body}");
    assert!(body.contains("ended without a closing message"));
    // And the claim itself is the agent's words, marked as such and judged by
    // nothing — no verdict, no score, no comparison against the evidence.
    assert!(body.contains("the agent's account: "));
    // Comments stripped first: the paragraph above this code *explains* why the
    // product does not judge, and the word "untruthful" in that explanation is
    // not a judgement reaching the page. Scanning the prose for the vocabulary
    // it is arguing against fails on code that is right, which it did.
    let rendered: String = body
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for judged in ["accurate", "matches", "contradicts", "truthful"] {
        assert!(
            !rendered.contains(judged),
            "the product does not grade the claim; it puts both on screen"
        );
    }
}

/// The rule to paste, and the one thing this surface must never grow.
///
/// Two properties. The first is that an absent offer is a
/// *sentence* — a blank where a rule belongs reads as a surface that failed
/// rather than one with nothing to say. The second is the whole reason the
/// feature hands over text instead of writing it: an agent on this machine runs
/// as the same user and can read the token this page uses, so a control that
/// edited `[policy]` would be a widening path reachable by the party the rules
/// govern.
#[test]
fn the_rule_is_offered_and_never_written() {
    let body = PAGE
        .split("function renderOffer(")
        .nth(1)
        .and_then(|r| r.split("\n}\n").next())
        .expect("the page renders an offer");

    // Both halves of the offer, and the absence as words.
    for want in ["i.offer", "o.covers", "o.file", "i.no_offer.sentence"] {
        assert!(body.contains(want), "the offer is missing `{want}`");
    }
    assert!(
        body.contains("esc(i.no_offer.sentence)"),
        "the reason is a sentence the daemon wrote, escaped like everything else"
    );

    // **Nothing writes a rule.** Every POST the page makes, checked against a
    // list — a new one that touched a policy file would have to be added here
    // deliberately, which is the point.
    let writes: Vec<&str> = PAGE
        .match_indices("/api/")
        .filter_map(|(at, _)| PAGE[at..].split(['`', '"', '\'', '$']).next())
        .filter(|r| r.contains("polic") || r.contains("rule") || r.contains("auto_allow"))
        .collect();
    assert!(
        writes.is_empty(),
        "the board must not reach a policy route: {writes:?}"
    );
    assert!(
        !PAGE.contains(r#"data-act="writerule""#) && !PAGE.contains("auto_allow\", {"),
        "no control writes a permission rule"
    );

    // The rule stays on the screen. A toast is not a fallback for a line
    // somebody types into a file on another machine.
    assert!(
        PAGE.contains("user-select: all"),
        "the rule has to be selectable where there is no clipboard"
    );
    assert!(
        PAGE.contains("no clipboard here"),
        "and say so rather than reporting a copy that did not happen"
    );
}

/// The escaping guarantee, after it stopped being a property of one file.
///
/// `every_value_read_off_the_api_is_escaped` scans the page for `${a.bare}`
/// interpolations. That works because all the interpolation is in one file it
/// can read — and rendering is moving into the daemon, where a grep would be a
/// weaker guard than the one it replaces.
///
/// So the guarantee is a type there, and this checks the two things a type
/// cannot check about itself: that the escape hatch still takes `&'static str`,
/// and that nobody has added a second way in.
#[test]
fn the_daemon_renderer_cannot_be_handed_an_unescaped_value() {
    let render = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render.rs"),
    )
    .expect("src/render.rs");

    // The hatch is `&'static str`. A value off the API, out of a repository or
    // out of a model is allocated at runtime and cannot satisfy it — that is
    // the whole guarantee, and widening this signature would delete it.
    assert!(
        render.contains("pub fn raw(s: &'static str) -> Self"),
        "Html::raw no longer takes `&'static str`; untrusted text can reach markup"
    );

    // And there is exactly one of it. A second constructor over `&str` or
    // `String` would be a way in that compiles.
    let constructors = render.matches("-> Self {").count();
    assert!(
        constructors > 0,
        "the renderer has no constructors at all, which cannot be right"
    );
    for forbidden in [
        "pub fn raw(s: &str)",
        "pub fn raw(s: String)",
        "pub fn raw_str",
        "impl From<String> for Html",
        "impl From<&str> for Html",
    ] {
        assert!(
            !render.contains(forbidden),
            "`{forbidden}` is a second way into markup that skips escaping"
        );
    }

    // `IntoHtml` must not gain a pass-through for a plain string: that is the
    // same hole wearing a trait's clothes.
    assert!(
        !render.contains("fn into_html(self) -> Html {\n        Html(self"),
        "IntoHtml passes a raw string straight through"
    );
}

/// Escaping works, checked by rendering the things that have actually bitten.
#[test]
fn the_renderer_escapes_what_repositories_and_models_produce() {
    use devplane::render::Html;
    for (input, must_not_contain) in [
        (r#"<img src=x onerror=alert(1)>"#, "<img"),
        (r#""onmouseover=""#, "\"onmouseover"),
        (r#"</script><script>"#, "</script>"),
        ("a & b", "a & b"),
        ("it's", "it's"),
    ] {
        let rendered = Html::text(input);
        assert!(
            !rendered.as_str().contains(must_not_contain),
            "`{input}` survived escaping as `{}`",
            rendered.as_str()
        );
    }

    // And the macro escapes its arguments while trusting its own literal.
    let name = r#"<b>evil</b>"#;
    let out = devplane::markup!("<span>{}</span>", name);
    assert!(out.as_str().starts_with("<span>"), "the literal is trusted");
    assert!(
        !out.as_str().contains("<b>"),
        "markup! let an argument through unescaped: {}",
        out.as_str()
    );

    // Html passed in is already safe and is not double-escaped.
    let inner = devplane::markup!("<i>{}</i>", "x & y");
    let outer = devplane::markup!("<p>{}</p>", inner);
    assert!(outer.as_str().contains("<i>"), "Html was re-escaped");
    assert!(
        outer.as_str().contains("&amp;"),
        "the text lost its escaping"
    );
}

// ── The rail, and the promises a list of offers has to keep ─────────────────

/// A rail entry offers a surface, and every offer can be taken.
///
/// **This is where "an offered action is an implemented action" is easiest to
/// break by accident.** Navigation is a list, a list is cheap to add to, and an
/// entry that looks enabled and does nothing is the same defect class as a
/// wrong answer — it just looks like a feature.
#[test]
fn no_rail_entry_offers_a_surface_that_cannot_be_opened() {
    let rail_start = PAGE.find("<nav class=\"rail\"").expect("the rail exists");
    let rail = &PAGE[rail_start..PAGE[rail_start..].find("</nav>").unwrap() + rail_start];

    // Every link names a section that exists in the page.
    let mut links = 0;
    for (at, _) in rail.match_indices("data-surface=\"") {
        let rest = &rail[at + 14..];
        let id = &rest[..rest.find('"').expect("a closing quote")];
        assert!(
            PAGE.contains(&format!("id=\"{id}\"")),
            "the rail offers `{id}`, which is not a section in the page"
        );
        links += 1;
    }
    // Four since the gate surface went with the permission gate it reported on.
    assert!(links >= 4, "the rail lists only {links} reachable surfaces");

    // And an unbuilt surface is not a control: not a link, not a button, so a
    // keyboard never lands on an offer that cannot be taken.
    assert!(
        rail.contains("<span class=\"off\">"),
        "no unbuilt surface is marked — either all six are built, or one is \
         being offered as though it were"
    );
    let off_start = rail.find("<span class=\"off\">").expect("an unbuilt entry");
    let off = &rail[off_start
        ..rail[off_start..]
            .find("</span>\n")
            .map_or(rail.len(), |i| i + off_start)];
    for control in ["<a ", "href=", "<button", "tabindex"] {
        assert!(
            !off.contains(control),
            "an unbuilt rail entry contains `{control}`, which puts it in the tab order"
        );
    }
    // It says what is missing rather than looking disabled and silent.
    assert!(
        off.contains("not built"),
        "an unbuilt entry does not say so"
    );
}

/// An empty surface and an unavailable one are different sentences.
#[test]
fn an_empty_surface_does_not_read_as_an_unavailable_one() {
    let render = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render.rs"),
    )
    .expect("src/render.rs");
    let empty = "Nothing has been decided about this yet";
    let unavailable = "not built";
    assert!(
        render.contains(empty),
        "an empty surface says nothing at all"
    );
    assert!(
        PAGE.contains(unavailable),
        "an unbuilt surface says nothing"
    );
    assert_ne!(empty, unavailable, "the two states share a sentence");
    // The one that means "there is nothing here" must not appear where the one
    // that means "this does not exist yet" belongs, and the reverse.
    assert!(
        !render.contains(unavailable),
        "the renderer describes data as unbuilt"
    );
}

// ── Type, figures and motion ────────────────────────────────────────────────

/// Prose is proportional; monospace is kept for what earns it.
///
/// **This had no task and no check until an analysis pass found it**, despite
/// being a stated part of the design. Everything with a *measurement* behind it
/// got tests; the design rules got none — which is the bias that produces a
/// technically-complete interface that looks wrong.
#[test]
fn monospace_is_kept_for_what_earns_it_and_numbers_are_tabular() {
    // The body sets prose, and it is not monospace.
    let body = PAGE
        .find("  body {")
        .map(|i| &PAGE[i..i + 600])
        .expect("a body rule");
    assert!(
        !body.contains("font: 13px/1.45 ui-monospace"),
        "the whole page is still monospace; prose costs ~15% of the line for nothing"
    );
    assert!(
        body.contains("system-ui"),
        "prose is not set in the system's own proportional stack"
    );

    // Monospace is declared for the three things that earn it — text somebody
    // else wrote, identifiers, and numbers in a column.
    for earns in [".row .name", ".item .d", "code", "kbd"] {
        assert!(
            PAGE.contains(earns),
            "`{earns}` is not in the monospace list; it carries text nobody here wrote"
        );
    }

    // A column of numbers is tabular, or it cannot be scanned — which is the
    // whole reason it is a column rather than a sentence.
    let tabular = PAGE
        .find("font-variant-numeric: tabular-nums")
        .map(|i| &PAGE[PAGE[..i].rfind('\n').unwrap_or(0)..i])
        .expect("a tabular-nums rule");
    for column in [".cost", ".gauge", ".row .since"] {
        assert!(
            tabular.contains(column) || PAGE.contains(&format!("{column}, ")),
            "`{column}` is a column of numbers and is not tabular"
        );
    }
}

/// Nothing depends on movement to be understood.
#[test]
fn no_state_is_carried_by_motion_and_reduced_motion_is_honoured() {
    // The strong form first: there is no animation to reduce. A changed number
    // changes; no row spins because it is working.
    for moving in ["@keyframes", "animation:", "animation-name"] {
        assert!(
            !PAGE.contains(moving),
            "`{moving}` is in the page; a state conveyed by movement is a state \
             a still screenshot and a reduced-motion reader both lose"
        );
    }
    // And the preference is honoured anyway, so it stays true rather than
    // being true by accident — and so a reader can see it was decided.
    assert!(
        PAGE.contains("prefers-reduced-motion"),
        "the page never mentions the preference it happens to satisfy"
    );
}

/// The default surface shows the inbox, and the header's count agrees with it.
///
/// **Written because it broke silently.** The board is the one surface made of
/// two sections, and renaming one of them left the "is this the board" check
/// naming the old id — so the inbox vanished from the default surface while the
/// header went on counting *"1 need you"*. Every test passed; a screenshot
/// found it.
#[test]
fn the_default_surface_shows_both_of_the_boards_sections() {
    let js = PAGE
        .find("const onBoard")
        .map(|i| &PAGE[i..i + 200])
        .expect("the board's two-section rule");

    // Whatever the board surface is called, the rail and this rule must agree.
    // Read from the rail specifically: the sections carry `data-surface` too,
    // as the flag saying whether they are the one showing.
    let rail = PAGE
        .find("<nav class=\"rail\"")
        .map(|i| &PAGE[i..i + PAGE[i..].find("</nav>").expect("the rail closes")])
        .expect("the rail exists");
    let rail_id = rail
        .split("data-surface=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .expect("the rail's first entry");
    assert!(
        js.contains(&format!("which === \"{rail_id}\"")),
        "the rail calls the default surface `{rail_id}` and the code checks for something else"
    );
    assert!(
        js.contains("id === \"needs\""),
        "the default surface no longer includes the inbox"
    );

    // And the fallback lands on a surface that exists.
    let fallback = PAGE
        .split("SURFACES.includes(want) ? want : \"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .expect("a fallback surface");
    assert!(
        PAGE.contains(&format!("\"{fallback}\"")) && PAGE.contains(&format!("id=\"{fallback}\"")),
        "the fallback surface `{fallback}` is not a section in the page"
    );
}

/// **The page must read a command's outcome, not the pair of fields it replaced.**
///
/// This is about the **work gate** — the project's own checks — which survives
/// the permission gate's removal and is the thing a done certificate rests on.
///
/// It is here because the change that introduced the outcome enum compiled,
/// passed the suite, and left the board silently rendering a cross beside every
/// command in every gate: JavaScript reading a field that no longer exists gets
/// `undefined` and says nothing about it.
#[test]
fn the_gate_view_reads_the_structured_outcome() {
    assert!(
        !PAGE.contains("c.exit_code") && !PAGE.contains("c.timed_out"),
        "the page still reads the fields the record no longer has, so every \
         command renders as though it failed"
    );
    assert!(
        PAGE.contains("c.outcome"),
        "the page does not read a command's outcome at all"
    );
}

/// A tick and a cross cannot say four things.
///
/// Exited, timed out, never started and could-not-be-determined are four
/// different sentences, and only the first is a verdict about the code — a
/// missing binary is a broken check, not a broken change.
#[test]
fn every_command_outcome_has_a_word_the_page_can_say() {
    for kind in ["timed_out", "never_started"] {
        assert!(
            PAGE.contains(kind),
            "the page cannot distinguish `{kind}` from an ordinary failure"
        );
    }
    for phrase in [
        "never started",
        "could not be determined",
        "ran out of time",
    ] {
        assert!(
            PAGE.contains(phrase),
            "no words for an outcome that is not a verdict: {phrase}"
        );
    }
}

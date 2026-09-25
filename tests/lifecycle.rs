//! Host start-up order, checked over the source: order is not observable from
//! the store afterwards.

fn source(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The body of one `fn`, up to the next top-level `fn`.
fn body<'a>(text: &'a str, signature: &str) -> &'a str {
    let start = text
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` is not in the source"));
    let rest = &text[start + signature.len()..];
    let end = rest
        .find("\npub async fn ")
        .or_else(|| rest.find("\npub fn "))
        .or_else(|| rest.find("\nasync fn "))
        .unwrap_or(rest.len());
    &rest[..end]
}

/// The spool is filed before anything is pruned, so retention cannot age out the
/// run a spooled decision belongs to. The halves live in different files.
#[test]
fn the_spool_is_filed_before_anything_is_pruned() {
    let cli = source("src/cli/mod.rs");
    let serve = body(&cli, "pub(crate) async fn cmd_serve(");
    let drain = serve
        .find("drain_decision_spool(")
        .expect("cmd_serve drains the spool");
    let host = serve
        .find("host::serve(")
        .expect("cmd_serve hands over to the host");
    assert!(
        drain < host,
        "the spool is drained after the host starts serving"
    );

    let host = source("src/host.rs");
    let serve = body(&host, "pub async fn serve(");
    let retention = serve
        .find("poller::retention(")
        .expect("serve runs retention");
    assert!(
        !serve.contains("drain_decision_spool"),
        "the drain moved inside serve, which makes the first check nearly vacuous — \
         serve is a tail call"
    );
    let first_task = serve
        .find("tokio::spawn(")
        .expect("serve spawns its periodic tasks");
    assert!(
        retention < first_task,
        "retention runs after the periodic tasks are up, so a tail could fold in a \
         row that a prune is about to drop"
    );

    let poller = source("src/poller.rs");
    let retention = body(&poller, "pub async fn retention(");
    assert!(
        !retention.contains("sleep(") && !retention.contains("interval("),
        "retention is a startup pass, not a loop: {retention}"
    );
}

/// Every periodic task spawned by `serve` is declared with what it looks at, and
/// every declared task still runs.
#[test]
fn every_periodic_job_says_why_it_cannot_be_a_reducer() {
    // (spawned function, what it goes and looks at)
    const DECLARED: &[(&str, &str)] = &[
        (
            "poller::tail",
            "rows other processes appended to the store since the last mark",
        ),
        (
            "poller::run",
            "the vendor's session roster and the process table",
        ),
        (
            "poller::stall_sweeper",
            "the notification half only — the stalled state is computed at read",
        ),
        (
            "poller::expiry_sweeper",
            "an ask past its deadline, whose ending is a row with the authority `timer`",
        ),
        (
            "poller::pull_requests",
            "the forge, for pull requests this host opened",
        ),
        (
            "poller::gate_watch",
            "the gate command, run once at start and again on a settings change",
        ),
        (
            "poller::forge_watch",
            "the forge, for issues and reviews assigned to the person",
        ),
        (
            "poller::tree_watch",
            "each open change's worktree, for the tree digest verified is checked against",
        ),
        (
            "observe::opencode::watch",
            "a vendor's own event feed, connected out to",
        ),
    ];
    let host = source("src/host.rs");
    let serve = body(&host, "pub async fn serve(");
    let spawned: Vec<&str> = serve
        .match_indices("tokio::spawn(crate::")
        .map(|(i, _)| {
            let rest = &serve[i + "tokio::spawn(crate::".len()..];
            rest.split('(').next().unwrap().trim()
        })
        .collect();
    assert!(
        spawned.len() >= 5,
        "the reader found only {spawned:?}; the shape of serve changed"
    );
    for s in &spawned {
        assert!(
            DECLARED.iter().any(|(name, _)| name == s),
            "`{s}` is spawned by the host and declares nothing about what it goes and looks at; \
             add it to the list with its reason, or make it a reducer"
        );
    }
    for (name, why) in DECLARED {
        assert!(
            spawned.contains(name),
            "`{name}` is declared ({why}) and no longer spawned; remove the row"
        );
        assert!(!why.is_empty());
    }
}

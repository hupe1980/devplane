//! The documentation and the parser have to agree.
//!
//! `vibeplane.toml` is read with `deny_unknown_fields`, so a single
//! designed-but-unbuilt key in a reference produces a file that does not load
//! *at all* — and that takes the repository's permission rules down with it. It
//! has happened twice: five sections that did not exist, and later an `on =`
//! key for standing pipelines. Both times the reference read as authoritative
//! and could not be pasted.
//!
//! A configuration reference that cannot be pasted is worse than none, so every
//! example in the README and on the documentation site is checked the way a
//! public API is: it must parse, it must mean something, and it must be a
//! configuration `vibeplane check` would accept.

use std::path::{Path, PathBuf};

/// One fenced ```toml block, with whatever file its first comment names.
struct Block {
    /// Where it came from, for a failure message somebody can act on.
    source: String,
    /// The file the block is an example *of*, from its own leading comment —
    /// which is there for the reader anyway. Empty means `vibeplane.toml`,
    /// because that is what most of this documentation is about.
    names: String,
    body: String,
}

fn blocks_in(source: &str, text: &str) -> Vec<Block> {
    text.split("```toml")
        .skip(1)
        .enumerate()
        .map(|(n, block)| {
            let body = block.split("```").next().unwrap_or("").to_string();
            let names = body
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .filter(|l| l.starts_with('#'))
                .map(|l| l.trim_start_matches('#').trim().to_string())
                .unwrap_or_default();
            // Named by its own comment where it has one, and by its shape
            // otherwise: an example that opens `[agents.…]` is an `agents.toml`
            // whether or not somebody wrote the path above it.
            let names = match names.is_empty() && body.trim_start().starts_with("[agents.") {
                true => "agents.toml".to_string(),
                false => names,
            };
            Block {
                source: format!("{source} (block {})", n + 1),
                names,
                body,
            }
        })
        .collect()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every `toml` block under a directory of markdown, recursively.
fn blocks_under(dir: &Path) -> Vec<Block> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(blocks_under(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            out.extend(blocks_in(&path.display().to_string(), &text));
        }
    }
    out
}

/// Checks one set of blocks, and returns how many were project configurations.
fn check(blocks: Vec<Block>) -> usize {
    let mut configs = 0;

    for b in blocks {
        // A block whose own comment calls it a fragment illustrates one key in
        // context and is not a whole file. It is still held to being valid
        // TOML, so a typo in an example is still a failing test — and the
        // comment is there for the reader, which is what keeps the escape
        // hatch honest.
        if b.names.contains("fragment") {
            assert!(
                toml::from_str::<toml::Value>(&b.body).is_ok(),
                "{}: this fragment is not valid TOML:\n{}",
                b.source,
                b.body
            );
            continue;
        }

        if b.names.contains("agents.toml") {
            // A different file with a different shape: it has to add the agent
            // it says it does, and pin it.
            let home = std::env::temp_dir().join(format!(
                "vp-doc-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir_all(&home).unwrap();
            std::fs::write(home.join("agents.toml"), &b.body).unwrap();

            let added = vibeplane::acp::user_agents(&home);
            assert!(
                !added.is_empty(),
                "{}: this `agents.toml` example adds no agent at all:\n{}",
                b.source,
                b.body
            );
            assert!(
                added.iter().all(|a| a.command.contains('@')),
                "{}: the documentation should show an agent pinned, not floating: {:?}",
                b.source,
                added.iter().map(|a| &a.command).collect::<Vec<_>>()
            );
            std::fs::remove_dir_all(&home).ok();
            continue;
        }

        let parsed = toml::from_str::<vibeplane::core::ProjectConfig>(&b.body);
        assert!(
            parsed.is_ok(),
            "{}: does not parse as vibeplane.toml:\n{}\n{}",
            b.source,
            parsed.unwrap_err(),
            b.body
        );
        let config = parsed.unwrap();

        // A block that parses because every key in it was ignored would pass
        // the line above and teach nobody.
        assert_ne!(
            config,
            vibeplane::core::ProjectConfig::default(),
            "{}: parses to nothing at all:\n{}",
            b.source,
            b.body
        );

        // And it must be a configuration the product would accept. Showing one
        // it refuses is worse than showing none: the reader follows the example
        // and then `vibeplane check` tells them they are wrong.
        let fatal: Vec<String> = config
            .validate()
            .into_iter()
            .filter(|p| p.fatal)
            .map(|p| p.to_string())
            .collect();
        assert!(
            fatal.is_empty(),
            "{}: shows a configuration `vibeplane check` refuses: {fatal:?}\n{}",
            b.source,
            b.body
        );

        configs += 1;
    }
    configs
}

#[test]
fn every_example_in_the_readme_is_one_that_works() {
    // The crate's own README is a symlink to the repository's, so this is the
    // document that ships to crates.io.
    const README: &str = include_str!("../README.md");
    let configs = check(blocks_in("README.md", README));
    assert!(configs >= 1, "the README shows no vibeplane.toml at all");
}

#[test]
fn every_example_on_the_documentation_site_is_one_that_works() {
    // `site/` is committed, so unlike the internal notes this is unconditional.
    // It is also where the configuration reference now lives, which makes it
    // the copy most likely to drift.
    let dir = repo_root().join("site/content");
    assert!(
        dir.is_dir(),
        "{} is missing; the check is looking in the wrong place",
        dir.display()
    );
    let configs = check(blocks_under(&dir));
    assert!(
        configs >= 8,
        "only {configs} vibeplane.toml examples found on the site; \
         the reference pages are the ones that have to be right"
    );
}

#[test]
fn every_example_in_the_internal_notes_is_one_that_works() {
    // Those notes are not committed, so this skips rather than fails on a clean
    // checkout: a suite that goes red when you clone is a suite nobody trusts.
    let dir = repo_root().join("concepts");
    if !dir.is_dir() {
        eprintln!("no concepts/ in this checkout; nothing to check");
        return;
    }
    check(blocks_under(&dir));
}

/// Every key a project can set appears in the configuration reference.
///
/// The sibling tests check that what the docs *show* parses. This checks the
/// other direction — that what the code *accepts* is shown — and the two failure
/// modes are opposite and both silent. A key that is documented and not read is
/// a setting somebody changes and watches do nothing (`[github] squash` was one,
/// for the life of the project). A key that is read and not documented is a
/// feature nobody can find (`always_ask`, the third permission list, was
/// another).
///
/// The writable surface is enumerated by serialising a default config rather
/// than by a hand-written list, so adding a field to the struct fails this until
/// the reference mentions it.
#[test]
fn every_key_a_project_can_set_is_in_the_reference() {
    // **Read from the source, not from a serialised default.** This used to
    // serialise `ProjectConfig::default()` and take the keys off the lines that
    // came out — which silently covered only the fields that are *not*
    // `Option`, because TOML omits a `None`. Most of the interesting surface is
    // optional, so the check had been passing on a fraction of what it claimed
    // and two new keys slipped straight through it.
    //
    // A struct that derives `Deserialize` is one a person can write; one that
    // only derives `Serialize` is something Vibeplane reports back, like a
    // `Problem`. That distinction is the whole filter, and it maintains itself.
    let src =
        std::fs::read_to_string(repo_root().join("src/core/config.rs")).expect("the config module");

    let mut keys: Vec<String> = Vec::new();
    for block in src.split("pub struct ").skip(1) {
        let Some(header_end) = block.find('{') else {
            continue;
        };
        // The derive list sits immediately above the struct, so it is the tail
        // of the previous block — found by looking back from this one's name.
        let name_at = src
            .find(&format!("pub struct {}", block[..header_end].trim()))
            .unwrap_or(0);
        let preamble = &src[name_at.saturating_sub(200)..name_at];
        if !preamble.contains("Deserialize") {
            continue;
        }
        let body_end = block.find("\n}").unwrap_or(block.len());
        for line in block[header_end..body_end].lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("pub ")
                && let Some((field, _)) = rest.split_once(':')
                && field.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                && !field.is_empty()
            {
                keys.push(field.to_string());
            }
        }
    }
    keys.sort();
    keys.dedup();
    assert!(
        keys.len() > 20,
        "the config surface looks too small to be right: {keys:?}"
    );

    // The references a person is sent to. The README is the sales page and
    // shows a subset on purpose, so it is not one of them.
    let sources: Vec<String> = ["site/content/docs/configuration.md", "concepts/GATES.md"]
        .iter()
        .filter_map(|p| std::fs::read_to_string(repo_root().join(p)).ok())
        .collect();
    assert!(!sources.is_empty(), "no configuration reference found");

    let undocumented: Vec<&String> = keys
        .iter()
        .filter(|k| !sources.iter().any(|s| s.contains(k.as_str())))
        .collect();
    assert!(
        undocumented.is_empty(),
        "a project can set these and the configuration reference never mentions them: {undocumented:?}"
    );
}

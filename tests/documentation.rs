//! The documentation and the parser have to agree.
//!
//! `devplane.toml` is read with `deny_unknown_fields`, so a single
//! designed-but-unbuilt key in a reference produces a file that does not load
//! *at all* — and that takes the repository's permission rules down with it. It
//! has happened twice: five sections that did not exist, and later an `on =`
//! key for standing pipelines. Both times the reference read as authoritative
//! and could not be pasted.
//!
//! A configuration reference that cannot be pasted is worse than none, so every
//! example in the README and on the documentation site is checked the way a
//! public API is: it must parse, it must mean something, and it must be a
//! configuration `devplane check` would accept.

use std::path::{Path, PathBuf};

/// One fenced ```toml block, with whatever file its first comment names.
struct Block {
    /// Where it came from, for a failure message somebody can act on.
    source: String,
    /// The file the block is an example *of*, from its own leading comment —
    /// which is there for the reader anyway. Empty means `devplane.toml`,
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
            // `reference/` is third-party documentation somebody else wrote,
            // fetched by `scripts/fetch-reference.sh`. It is full of `toml`
            // blocks — MCP server stanzas, the vendor's own settings examples —
            // and not one of them is a `devplane.toml`, so holding them to this
            // parser asserts that other projects use our schema. It moved under
            // `concepts/` on 2026-09-18 and this test went red the same minute,
            // which is the check working: the corpus is evidence to grep, never
            // an example to validate.
            if path.file_name().and_then(|n| n.to_str()) == Some("reference") {
                continue;
            }
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

            let added = devplane::acp::user_agents(&home);
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

        let parsed = toml::from_str::<devplane::core::ProjectConfig>(&b.body);
        assert!(
            parsed.is_ok(),
            "{}: does not parse as devplane.toml:\n{}\n{}",
            b.source,
            parsed.unwrap_err(),
            b.body
        );
        let config = parsed.unwrap();

        // A block that parses because every key in it was ignored would pass
        // the line above and teach nobody.
        assert_ne!(
            config,
            devplane::core::ProjectConfig::default(),
            "{}: parses to nothing at all:\n{}",
            b.source,
            b.body
        );

        // And it must be a configuration the product would accept. Showing one
        // it refuses is worse than showing none: the reader follows the example
        // and then `devplane check` tells them they are wrong.
        let fatal: Vec<String> = config
            .validate()
            .into_iter()
            .filter(|p| p.fatal)
            .map(|p| p.to_string())
            .collect();
        assert!(
            fatal.is_empty(),
            "{}: shows a configuration `devplane check` refuses: {fatal:?}\n{}",
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
    assert!(configs >= 1, "the README shows no devplane.toml at all");
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
        "only {configs} devplane.toml examples found on the site; \
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
    // only derives `Serialize` is something Devplane reports back, like a
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

    // **A bare substring is not a mention.** This used to ask whether the page
    // *contained* the key's name, which quietly passes for every short, common
    // word: a key called `only` was "documented" by the phrase "a ceiling in
    // dollars only bites", and `max`, `file` and `run` are the same trap. The
    // reference writes a key in backticks or as a TOML assignment, and nothing
    // else counts — so a new key has to be written down rather than coincide
    // with English.
    let documented = |k: &str| {
        let forms = [
            format!("`{k}`"),
            format!("{k} ="),
            format!("{k}="),
            // A section is written as its header, not as a bare key.
            format!("[{k}]"),
            format!("[{k}."),
            format!(".{k}]"),
            // A nested key is documented by its path: `findings.file`.
            format!(".{k}`"),
        ];
        sources
            .iter()
            .any(|s| forms.iter().any(|f| s.contains(f.as_str())))
    };
    let undocumented: Vec<&String> = keys.iter().filter(|k| !documented(k)).collect();
    assert!(
        undocumented.is_empty(),
        "a project can set these and the configuration reference never mentions them: {undocumented:?}"
    );
}

/// The release workflow is hand-written rather than generated by `dist init`
/// (`allow-dirty = ["ci"]`), so its build matrix repeats the target list in
/// `[workspace.metadata.dist]`. Nothing but this keeps the two equal, and a
/// target that is declared and never built is missing from the release with no
/// error anywhere.
#[test]
fn the_release_workflow_builds_every_target_the_manifest_declares() {
    let manifest = std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml");
    let manifest: toml::Value = toml::from_str(&manifest).expect("Cargo.toml parses");
    let declared: Vec<String> = manifest["workspace"]["metadata"]["dist"]["targets"]
        .as_array()
        .expect("[workspace.metadata.dist] targets")
        .iter()
        .map(|t| t.as_str().expect("a target triple").to_string())
        .collect();

    let workflow = std::fs::read_to_string(repo_root().join(".github/workflows/release.yml"))
        .expect("release.yml");
    let mut built: Vec<String> = workflow
        .lines()
        .filter_map(|l| l.trim().strip_prefix("target: "))
        .map(|t| t.trim().to_string())
        .collect();
    built.sort();
    built.dedup();

    let mut declared_sorted = declared.clone();
    declared_sorted.sort();
    assert_eq!(
        declared_sorted, built,
        "[workspace.metadata.dist] targets and the release.yml build matrix disagree"
    );
}

/// **The Spec Kit version this integration was read against is recorded, and
/// something notices when the installed copy moves past it.**
///
/// Everything Devplane knows about Spec Kit — the extensions file, the twenty
/// hook points, what `optional: false` obliges an agent to do, that a
/// `condition` is skipped silently by every agent — was read from an installed
/// copy. None of it is announced anywhere this product watches. So the version
/// is a constant, and this reports when the checkout has a different one.
///
/// **It reports rather than fails**, on the npm-pin precedent: the version does
/// not move on its own, so what a difference produces is a decision — re-read
/// the shape, or move the constant — and not a broken build.
#[test]
fn the_spec_kit_version_this_was_read_against_is_recorded() {
    use devplane::core::spec::SPEC_KIT_READ_AGAINST;

    assert!(
        SPEC_KIT_READ_AGAINST
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
        "the recorded version must be a version: {SPEC_KIT_READ_AGAINST}"
    );

    // **Nothing here compares it to the installed copy**, and the reason is the
    // rule this repository enforces on itself: the published tree may not point
    // at a path a reader does not have. Spec Kit's own scratch directory is
    // gitignored, so the comparison lives in `scripts/concepts-check.sh`, which
    // is a recipe run from a working checkout rather than shipped code.
}

/// **The published npm version cannot diverge from the crate version**, and
/// the package cannot acquire a thing that phones home.
///
/// Both are stated in the specification as requirements and neither was
/// checked. They are one test because they are one property of the same
/// configuration: the package is **generated by `dist` from this manifest**, so
/// the version is the crate's by construction and the install-time behaviour is
/// whatever the manifest permits. The failure this guards is somebody replacing
/// the generated package with a hand-written `package.json` — at which point
/// both properties become somebody's memory.
#[test]
fn the_npm_package_is_generated_from_the_manifest_and_sends_nothing() {
    let manifest = std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml");
    let parsed: toml::Value = toml::from_str(&manifest).expect("Cargo.toml parses");
    let dist = &parsed["workspace"]["metadata"]["dist"];

    let installers: Vec<&str> = dist["installers"]
        .as_array()
        .expect("[workspace.metadata.dist] installers")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        installers.contains(&"npm"),
        "the npm door is what this feature is; without the installer there is no package"
    );

    // **The updater is the thing that phones home.** `dist` can ship a shim
    // that checks for a newer release at run time; this product says in its own
    // documentation that it sends nothing anywhere, and that sentence is only
    // true while this is false.
    assert_eq!(
        dist.get("install-updater").and_then(toml::Value::as_bool),
        Some(false),
        "install-updater must be explicitly false: an updater checks a remote for a version, \
         which is the one network call the package promises not to make"
    );

    // And the version cannot be written by hand anywhere: no checked-in
    // package.json may claim the published name.
    for rel in ["package.json", "npm/package.json", "ui/package.json"] {
        let path = repo_root().join(rel);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let json: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        assert_ne!(
            json.get("name").and_then(|v| v.as_str()),
            Some("devplane"),
            "{rel} declares the published package name, so its version is a second home for \
             the crate version. The package is generated by `dist` from Cargo.toml"
        );
    }

    // The release workflow builds it with `dist` rather than publishing a
    // directory somebody assembled.
    let workflow = std::fs::read_to_string(repo_root().join(".github/workflows/release.yml"))
        .expect("release.yml");
    assert!(
        workflow.contains("dist build --artifacts=global"),
        "the npm package must be built by `dist` from the manifest, or its version is not the \
         crate's by construction"
    );
}

/// Every rule spelling the permissions page calls **refused** is one the gate
/// actually reports, and every one it shows as working actually works.
///
/// The page is the only place a person learns which rule shapes are dead, and
/// it contradicted itself for a release: one section documented `!` exceptions
/// as honoured and scoped to their file — which is what the code does — while
/// the "rules that cannot work" table three screens down said Devplane did not
/// implement them and told the reader to keep such a rule in `settings.json`.
/// Both sentences were written from the code, at different times, and nothing
/// read either one afterwards.
///
/// So the table is executed. A row is a rule in backticks in the first column;
/// the check is that `problems()` has something to say about it. The inverse
/// half matters as much: a rule the page presents as ordinary must be clean,
/// or the page is teaching a spelling the gate will refuse.
#[test]
fn the_refused_rules_table_on_the_site_is_the_one_the_gate_refuses() {
    use devplane::core::policy::{Class, Rule};

    let page = std::fs::read_to_string(repo_root().join("site/content/docs/permissions.md"))
        .expect("the permissions page");

    // The table that follows the heading, up to the next one.
    let start = page
        .find("## Rules that cannot work are refused")
        .expect("the refused-rules heading");
    let table = &page[start..];
    let table = &table[..table.find("\nAnd one **warning**").unwrap_or(table.len())];

    let mut checked = 0;
    for line in table.lines().filter(|l| l.starts_with("| `")) {
        let cell = line
            .trim_start_matches("| ")
            .split(" |")
            .next()
            .unwrap_or("");
        for raw in cell.split(", ").filter_map(|s| {
            let s = s.trim();
            s.strip_prefix('`')
                .and_then(|s| s.split('`').next())
                .filter(|s| !s.is_empty())
        }) {
            // Every row is a deny-side rule now. The page used to carry
            // allow-side-only refusals and the branch that classified them
            // went with the allow list itself.
            let Some(rule) = Rule::parse(raw, Class::Deny) else {
                continue; // a rule that does not parse at all is refused enough
            };
            assert!(
                !rule.problems().is_empty(),
                "the permissions page lists `{raw}` as refused, and the gate has \
                 nothing to say about it — one of the two is out of date"
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 6,
        "only {checked} refused rules were checked; the table's shape changed \
         and this test stopped reading it"
    );

    // The other direction, on the spellings the page teaches as ordinary.
    for (raw, class) in [
        ("Bash(rm *)", Class::Deny),
        ("Read(.env)", Class::Deny),
        ("Edit(src/**)", Class::Allow),
        ("PowerShell(Remove-Item *)", Class::Deny),
        ("!Bash(git status *)", Class::Deny),
        ("mcp__github__get_*", Class::Deny),
    ] {
        let rule = Rule::parse(raw, class).expect("a documented rule parses");
        assert!(
            rule.problems().is_empty(),
            "the permissions page teaches `{raw}`, and the gate reports {:?}",
            rule.problems()
        );
    }
}

/// Wherever the published tree names the release the rule syntax was modelled
/// on, it is the number the binary holds.
///
/// It used to live in comments in three source files, in a shell script and in
/// four published pages — eight copies of one number, and nothing read any of
/// them. The harness that moved it is gone with the approval path, so the
/// number is frozen now; what is still worth protecting is the copying.
///
/// `SYNTAX_MODELLED_ON` is the authority. The scan is over **every** published
/// page rather than a list of four, because the previous version of this test
/// named its files and a fifth page could say anything it liked.
#[test]
fn every_page_that_names_the_gate_baseline_names_the_one_the_binary_holds() {
    let baseline = devplane::core::policy::SYNTAX_MODELLED_ON;
    let root = repo_root();
    // The phrasings the published tree actually uses, each followed by the
    // version. A page that invents a ninth phrasing is invisible here, which is
    // why the count is asserted too.
    //
    // Only the phrasings that state the **harness baseline**. "Built against
    // Claude Code 2.1.273" is a different fact — the release the product was
    // developed and read against — and the two are deliberately different
    // numbers, because one costs a grep and the other costs a signed-in agent
    // and real money. Collapsing them is the mistake this project already made
    // once; a test that collapses them teaches the same error with authority.
    let patterns = [
        "rule syntax modelled on Claude Code ",
        "last full run: ",
        "ran in full against ",
        "full run, against Claude Code ",
    ];
    let mut pages: Vec<PathBuf> = vec![root.join("README.md")];
    let mut stack = vec![root.join("site/content")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                pages.push(p);
            }
        }
    }
    let mut found = 0;
    for path in pages {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for pat in patterns {
            let mut from = 0;
            while let Some(i) = text[from..].find(pat) {
                let at = from + i + pat.len();
                let version: String = text[at..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                from = at;
                // Not every match is a version: "against Claude Code hooks".
                if version.split('.').count() < 3 {
                    continue;
                }
                let version = version.trim_end_matches('.');
                assert_eq!(
                    version, baseline,
                    "{rel} says the gate was verified against {version}; the binary says \
                     {baseline}. The baseline is frozen — change the page, not the \
                     constant."
                );
                found += 1;
            }
        }
    }
    assert!(
        found >= 1,
        "no page states the gate baseline any more; either the phrasings above went \
         stale or the number stopped being published, and this test now checks nothing"
    );
}

/// The agent-facing index names every command the binary has.
///
/// Agents consult agent-facing documentation 60.5 % of the time against 10.6 %
/// for ordinary technical docs, and this product's audience is people running
/// agents — so `llms.txt` is the page most likely to be *read* and the one
/// nothing was checking. A hand-written index of a thirty-command CLI drifts on
/// the first rename, and the drift is silent: the file still parses, still
/// reads well, and describes a binary that no longer exists.
///
/// So the list is checked against the source of truth rather than against a
/// copy of it. `clap` owns the command names; this reads them out of the same
/// enum the parser is built from.
#[test]
fn the_agent_facing_index_names_every_command() {
    let llms = include_str!("../site/static/llms.txt");
    let cli = include_str!("../src/cli/mod.rs");

    // The variants of `enum Command`, which is what `--help` prints and what a
    // person types. Read from the enum body only, so a struct field or a match
    // arm elsewhere in the file cannot be mistaken for a command.
    let body = cli
        .split_once("pub enum Command {")
        .expect("src/cli/mod.rs must declare `pub enum Command`")
        .1;
    let body = &body[..body.find("\n}").expect("the enum must close")];

    let mut missing = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        // A variant sits at one level of indentation and starts with a capital.
        if !line.starts_with("    ") || line.starts_with("     ") {
            continue;
        }
        let Some(name) = t
            .split(['{', '(', ','])
            .next()
            .map(str::trim)
            .filter(|n| n.chars().next().is_some_and(char::is_uppercase))
        else {
            continue;
        };
        // clap's default rename: `WorkStart` -> `work-start`, `Ls` -> `ls`.
        let mut kebab = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                kebab.push('-');
            }
            kebab.extend(c.to_lowercase());
        }
        // `hook` and `statusline` are typed by a hook, never by a person, and
        // the index says so rather than pretending they are user commands.
        if !llms.contains(&format!("`devplane {kebab}")) {
            missing.push(kebab);
        }
    }
    assert!(
        missing.is_empty(),
        "site/static/llms.txt does not name: {missing:?} — an agent reading it \
         would be told about a binary that does not exist"
    );
}

/// **The plugin manifests say what this crate is, and a reserved name would
/// make the marketplace unloadable.**
///
/// A plugin is the cheapest door this project has — two JSON files — and both
/// ways it can rot are silent. A version that trails the crate installs an
/// older story than the binary tells; a name on the vendor's reserved list
/// stops the marketplace loading entirely, and that list **grows**: names are
/// re-checked on every load, so a marketplace that worked last month can stop
/// working because somebody else reserved its name.
#[test]
fn the_plugin_manifests_match_this_crate() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let plugin: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("plugin/.claude-plugin/plugin.json"))
            .expect("the plugin manifest"),
    )
    .expect("valid JSON");
    let market: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(".claude-plugin/marketplace.json"))
            .expect("the marketplace manifest"),
    )
    .expect("valid JSON");

    assert_eq!(
        plugin["version"].as_str(),
        Some(env!("CARGO_PKG_VERSION")),
        "the plugin's version has drifted from the crate's"
    );

    // Reserved by the vendor for its own use, plus the package-manager names it
    // blocks in any casing. A marketplace called one of these is reported as
    // registered from an untrusted source.
    const RESERVED: &[&str] = &[
        "claude-code-marketplace",
        "claude-code-plugins",
        "claude-plugins-official",
        "claude-plugins-community",
        "claude-community",
        "anthropic-marketplace",
        "anthropic-plugins",
        "agent-skills",
        "anthropic-agent-skills",
        "knowledge-work-plugins",
        "life-sciences",
        "claude-for-legal",
        "claude-for-financial-services",
        "financial-services-plugins",
        "first-party-plugins",
        "claude-tag-plugins",
        "healthcare",
        "npm",
        "pip",
        "uv",
        "cargo",
        "github",
        "gh",
    ];
    for name in [market["name"].as_str(), plugin["name"].as_str()] {
        let name = name.expect("a name").to_ascii_lowercase();
        assert!(
            !RESERVED.contains(&name.as_str()),
            "`{name}` is reserved by the vendor; the marketplace would not load"
        );
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "`{name}` is not kebab-case"
        );
    }

    // Every entry points at a directory that is here, with a manifest in it.
    for entry in market["plugins"].as_array().expect("plugins") {
        let source = entry["source"].as_str().expect("a source");
        let dir = root.join(source.trim_start_matches("./"));
        assert!(
            dir.join(".claude-plugin/plugin.json").is_file(),
            "{source} has no plugin manifest, so the entry installs nothing"
        );
    }

    // The MCP server the plugin ships is this binary's own read-only surface.
    // Naming a different command here would ship a door into something else.
    let mcp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("plugin/mcp.json")).expect("the MCP config"),
    )
    .expect("valid JSON");
    assert_eq!(
        mcp["mcpServers"]["devplane"]["command"].as_str(),
        Some("devplane")
    );
    assert_eq!(
        mcp["mcpServers"]["devplane"]["args"][0].as_str(),
        Some("mcp"),
        "the plugin must start the read-only surface and nothing else"
    );
}

/// **Every skill the plugin ships is loadable, and says only what the binary
/// does.**
///
/// A skill with broken frontmatter is not reported as broken: it is silently
/// absent, so the failure looks like an agent that ignored an instruction.
/// `claude plugin validate` catches the shape by hand; this catches it on every
/// run, and adds the rule that tool has no way to know — a skill may only tell
/// an agent to run commands this binary actually has.
#[test]
fn every_skill_the_plugin_ships_is_loadable_and_runs_only_real_commands() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let skills = root.join("plugin/skills");
    let mut seen = 0;

    for entry in std::fs::read_dir(&skills).expect("the plugin ships skills") {
        let dir = entry.expect("a readable entry").path();
        if !dir.is_dir() {
            continue;
        }
        seen += 1;
        let folder = dir.file_name().unwrap().to_string_lossy().to_string();
        let body = std::fs::read_to_string(dir.join("SKILL.md"))
            .unwrap_or_else(|_| panic!("{folder} has no SKILL.md, so it loads nothing"));

        // Frontmatter, delimited exactly: the loader reads the first block and
        // a file that merely starts with prose is skipped without a word.
        let rest = body
            .strip_prefix("---\n")
            .unwrap_or_else(|| panic!("{folder}: SKILL.md must open with `---`"));
        let (front, _) = rest
            .split_once("\n---\n")
            .unwrap_or_else(|| panic!("{folder}: the frontmatter block is not closed"));

        let field = |key: &str| -> String {
            front
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{key}: ")))
                .unwrap_or_else(|| panic!("{folder}: SKILL.md declares no `{key}`"))
                .trim()
                .to_string()
        };

        let name = field("name");
        assert_eq!(
            name, folder,
            "a skill's name must be its directory, or the loader and the \
             marketplace disagree about what it is called"
        );
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{folder}: `{name}` is not kebab-case"
        );

        // The description is the whole of what an agent sees before deciding to
        // load the skill. One that does not say *when* to use it is a skill
        // that is never used.
        let description = field("description");
        assert!(
            description.len() < 1024,
            "{folder}: the description is {} characters; the loader truncates it",
            description.len()
        );
        assert!(
            description.to_ascii_lowercase().contains("use when"),
            "{folder}: the description must say when to use the skill: {description}"
        );

        // **And the commands it names exist.** A skill that tells an agent to
        // run something this binary does not have fails in somebody's session,
        // which is the most expensive place to find out.
        let help = String::from_utf8(
            std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
                .arg("--help")
                .output()
                .expect("the binary runs")
                .stdout,
        )
        .expect("help is text");
        for line in body.lines() {
            let Some(rest) = line.trim().strip_prefix("devplane ") else {
                continue;
            };
            let Some(word) = rest.split_whitespace().next() else {
                continue;
            };
            if word.starts_with('-') {
                continue;
            }
            assert!(
                help.contains(&format!("  {word}")),
                "{folder}: names `devplane {word}`, which this binary has no \
                 subcommand for"
            );
        }
    }

    assert!(
        seen >= 2,
        "expected the plugin's skills to be found, saw {seen}"
    );
}

/// **Somebody else's contract, pinned to the copy this repository read.**
///
/// The whole Spec Kit feature rests on four sentences in a file this project
/// does not own and cannot version. Each of them can change without a release
/// note, and each failure is silent in the worst way: a hook that is registered
/// correctly, committed, and never fires. A `condition` is the sharpest of
/// them — every command defers evaluation to a `HookExecutor` that does not
/// exist, so an entry carrying one looks right and does nothing.
///
/// This fails here, on a Spec Kit upgrade, rather than in somebody's workflow.
/// It skips when Spec Kit is not installed, because the contract is only worth
/// checking against a copy that is present.
#[test]
fn the_speckit_hook_contract_still_says_what_this_feature_relies_on() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let skill = root.join(".claude/skills/speckit-implement/SKILL.md");
    let Ok(body) = std::fs::read_to_string(&skill) else {
        // Not installed here. The feature still works; nothing can be checked.
        return;
    };

    // Quoted from the installed copy, not paraphrased — a paraphrase would keep
    // passing across exactly the rewording that matters.
    const CLAUSES: &[(&str, &str)] = &[
        (
            "hooks come from `.specify/extensions.yml`",
            "`.specify/extensions.yml` exists in the project root",
        ),
        (
            "the event this feature registers under",
            "hooks.after_implement",
        ),
        (
            "a non-empty condition is skipped, so our entry carries none",
            "skip the hook and leave condition evaluation to the HookExecutor",
        ),
        (
            "dots in the command name become hyphens",
            "replace dots (`.`) with hyphens (`-`)",
        ),
        (
            "a mandatory hook must actually be invoked and waited for",
            "you MUST actually invoke the hook and wait for it to finish",
        ),
        (
            "`enabled: false` turns a hook off",
            "Filter out hooks where `enabled` is explicitly `false`",
        ),
        (
            "an unparseable extensions.yml is reported rather than skipped",
            "do not skip silently",
        ),
    ];

    let mut gone = Vec::new();
    for (why, quote) in CLAUSES {
        if !body.contains(quote) {
            gone.push(format!("{why} — no longer says: {quote:?}"));
        }
    }
    assert!(
        gone.is_empty(),
        "Spec Kit's hook contract changed under this feature. Re-read \
         {skill:?} and `devplane speckit install` before trusting it:\n  {}",
        gone.join("\n  ")
    );

    // And the event we default to is one the installed copy actually reads.
    assert!(
        body.contains(&format!(
            "hooks.{}",
            devplane::core::spec::DEFAULT_HOOK_EVENT
        )),
        "`{}` is not an event this Spec Kit version looks for",
        devplane::core::spec::DEFAULT_HOOK_EVENT
    );
}

/// **The vendor's session vocabulary, pinned to the reference this code was
/// written from.**
///
/// Three bugs came out of one gap here: `status` is documented as `busy`,
/// `waiting` **or** `idle`, and the reducer had arms for two of them. The third
/// fell into the wrong one, so a session its own vendor reported as blocked on
/// a person was shown as waiting for a prompt and never reached the inbox.
///
/// Nothing in this repository could have noticed, because the code and its
/// tests agreed with each other and neither had read the list. So the list is
/// read: if the vendor adds a fourth status, a sixth thing to be blocked on, or
/// a new session state, this fails here rather than by silently mishandling it
/// on somebody's machine. It skips when the reference is not checked out.
#[test]
fn every_session_state_the_vendor_documents_is_one_this_build_handles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let Ok(reference) =
        std::fs::read_to_string(root.join("concepts/reference/claude-code/agent-view.md"))
    else {
        return;
    };

    // The row that defines `status`, read out of the field table rather than
    // remembered. Its wording is the vendor's: "one of `busy`, `waiting`, or
    // `idle`".
    let status_row = reference
        .lines()
        .find(|l| l.contains("`pid`, `status`"))
        .expect("the field table still documents `status`");
    let documented: Vec<&str> = ["busy", "waiting", "idle"]
        .into_iter()
        .filter(|v| status_row.contains(&format!("`{v}`")))
        .collect();
    assert_eq!(
        documented.len(),
        3,
        "the three status values this build handles are no longer the ones \
         documented; the row now reads: {status_row}"
    );
    // And nothing has been added beside them. Counted rather than matched,
    // because the point is to notice a value nobody here has thought about.
    let backticked = status_row.matches('`').count() / 2;
    assert_eq!(
        backticked, 5,
        "`pid`, `status` and three values is five quoted terms; the row now has \
         {backticked}, so something was added or removed: {status_row}"
    );

    // What a waiting session can be blocked on. Every one of these must map to
    // something `needs_human()` is true of — the reducer's own rule is that an
    // unrecognised value keeps its words rather than becoming *not urgent*.
    let waiting_row = reference
        .lines()
        .find(|l| l.contains("`waitingFor`"))
        .expect("the field table still documents `waitingFor`");
    for value in [
        "permission prompt",
        "input needed",
        "sandbox request",
        "worker request",
        "dialog open",
    ] {
        assert!(
            waiting_row.contains(value),
            "`{value}` is no longer documented as something a session waits on"
        );
        let parsed = devplane::core::event::WaitingFor::parse_roster(Some(value));
        assert!(
            devplane::core::RunState::Waiting(parsed).needs_human(),
            "`{value}` must reach a person"
        );
    }

    // The background `state` vocabulary, which decides a row the provider's own
    // daemon owns. Read from its own field-table row for the same reason.
    let state_row = reference
        .lines()
        .find(|l| l.contains("| `state`"))
        .expect("the field table still documents `state`");
    for value in ["working", "blocked", "done", "failed", "stopped"] {
        assert!(
            state_row.contains(&format!("`{value}`")),
            "`{value}` is no longer a documented session state: {state_row}"
        );
    }
    let state_terms = state_row.matches('`').count() / 2;
    assert_eq!(
        state_terms, 6,
        "`state` plus five values is six quoted terms; the row now has          {state_terms}, so the vocabulary changed: {state_row}"
    );

    // The sentence the whole inbox rests on, quoted because a change to it is a
    // change to what Devplane is entitled to claim.
    assert!(
        reference.contains("`blocked` always means the session needs something from you"),
        "the vendor no longer says `blocked` always means a person is needed, \
         so the inbox's central inference needs re-reading"
    );
}

/// **Every command this binary has is in the CLI reference, or is named here as
/// deliberately absent.**
///
/// `llms.txt` has had this guard since an agent was told about a binary that did
/// not exist; the page a *person* reads had none, and two commands shipped
/// without an entry. The exemption list is the point: a command is left out on
/// purpose and says why, rather than by nobody noticing.
///
/// **Hidden commands are checked too, and they were the hole.** This read
/// `--help`, so the moment a command was given `hide = true` it left the guard
/// entirely — the site could stop documenting it and nothing would notice.
/// `devplane mcp` is the case that matters: a person never types it and a
/// person absolutely has to configure it, so hiding it from a help listing is
/// right and dropping it from the reference is not. Hiding a command is now a
/// deliberate act with a written consequence rather than a way out of this
/// test.
#[test]
fn every_command_is_in_the_cli_reference_or_deliberately_not() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let page =
        std::fs::read_to_string(root.join("site/content/docs/cli.md")).expect("the CLI reference");

    // Not in the reference, each for a reason a reader would agree with.
    const UNDOCUMENTED: &[(&str, &str)] = &[
        (
            "statusline",
            "the status-line shim's own entry point. `devplane connect` installs \
             it and nobody types it",
        ),
        (
            "hook",
            "the `SessionStart` shim. `devplane connect` installs it and nobody \
             types it",
        ),
        ("help", "clap's builtin"),
    ];

    // Commands kept out of the help listing because they are surfaces for a
    // machine — and still owed an entry, because somebody has to configure them.
    const HIDDEN: &[&str] = &["mcp", "statusline", "hook"];

    let help = String::from_utf8(
        std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("--help")
            .output()
            .expect("the binary runs")
            .stdout,
    )
    .expect("help is text");

    // **Read from clap, not scraped from the text.** This parsed every
    // two-space-indented line after `Commands:` until the end of the output,
    // which was correct while `--help` ended there. It stopped being correct the
    // moment the help grew a block of grouped command names below the listing:
    // the parse read the group *headings* — `See`, `What`, `Start`, `Set`,
    // `The` — as commands and demanded a reference page for each.
    //
    // The command list has an exact source, and asking clap for it cannot drift
    // when the rendering changes again.
    let commands: Vec<String> = {
        use clap::CommandFactory;
        // **Built first**, so clap's own `help` subcommand exists. It is
        // generated during `build`, and before that it is not in the tree — it
        // used to be found by scraping the rendered flat listing, which is
        // gone.
        let mut cmd = devplane::cli::Cli::command();
        cmd.build();
        cmd.get_subcommands()
            .map(|s| s.get_name().to_string())
            .collect()
    };
    // And what the person actually **sees**, read from the rendered listing, so
    // "hidden" can still be checked against the surface rather than against the
    // type.
    //
    // **The listing is the grouped one now**, and there is no other: clap's
    // flat `Commands:` block was removed from the template because printing it
    // *and* the groups put the problem on the screen twice. This parse read
    // that block, so it found nothing the moment it went — caught by its own
    // vacuity assertion below, which is the whole reason that assertion exists.
    //
    // A command line is four-space indented under a two-space group heading;
    // the heading itself is prose and never matches a subcommand name.
    let known_names: std::collections::HashSet<&str> =
        commands.iter().map(String::as_str).collect();
    let listed: Vec<String> = help
        .lines()
        .filter_map(|l| {
            let name = l.strip_prefix("    ")?.split_whitespace().next()?;
            known_names.contains(name).then(|| name.to_string())
        })
        .collect();
    assert!(
        listed.len() > 20,
        "the parse found {} listed commands, so it is not reading the help",
        listed.len()
    );
    assert!(
        commands.len() > 20,
        "the parse found {} commands, so it is not reading the help",
        commands.len()
    );

    let mut missing = Vec::new();
    for c in &commands {
        if UNDOCUMENTED.iter().any(|(name, _)| name == c) {
            continue;
        }
        if !page.contains(&format!("devplane {c}")) {
            missing.push(c.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "site/content/docs/cli.md does not name: {missing:?} — a person reading \
         the reference would not know these exist"
    );

    // A hidden command is absent from the listing above, so it is checked here
    // by the only means left: asking the binary. `--help` on a name that is not
    // a command exits non-zero, which is what makes this a real check rather
    // than a list of strings.
    for name in HIDDEN {
        let ok = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .args([name, "--help"])
            .output()
            .expect("the binary runs")
            .status
            .success();
        assert!(
            ok,
            "`{name}` is listed as a hidden command and the binary does not have it"
        );
        assert!(
            !listed.iter().any(|c| c == name),
            "`{name}` is listed as hidden and appears in the help listing"
        );
        if UNDOCUMENTED.iter().any(|(n, _)| n == name) {
            continue;
        }
        assert!(
            page.contains(&format!("devplane {name}")),
            "site/content/docs/cli.md does not name the hidden command `{name}` — \
             hiding a command from `--help` is not a reason to stop documenting it, \
             and somebody has to configure this one"
        );
    }

    // And the exemptions are real commands, so the list cannot rot into a set of
    // names for things that no longer exist.
    for (name, why) in UNDOCUMENTED {
        // `help` is clap's own and is rendered without being a subcommand of
        // ours, so both sets are consulted.
        let known = commands.iter().any(|c| c == name)
            || listed.iter().any(|c| c == name)
            || HIDDEN.contains(name);
        assert!(
            known,
            "`{name}` is exempted from the reference ({why}) but is not a command"
        );
    }
}

/// **The site's group table and the CLI's own grouping cannot disagree.**
///
/// `devplane --help` sorts its thirty-five commands into five errands, and
/// `site/content/docs/cli.md` prints the same five as a table. They were two
/// hand-maintained copies of one list: moving a command between groups in the
/// code left the page asserting the old one, and nothing anywhere compared
/// them.
///
/// **An error a person can act on names the way out, and none of them leaks
/// the plumbing.**
///
/// `devplane show <typo>` has said *"`devplane ls --all` lists every session,
/// ids included"* since it shipped. Two paths did not: `dispatch --to <typo>`
/// named the problem and stopped, and `answer <typo>` printed a **serde error,
/// an internal route and the loopback port**, because the daemon answered 404
/// with plain text and the client decoded every body as JSON.
///
/// This holds the shape rather than the wording: a user-facing failure may not
/// contain the words a decode failure is made of.
#[test]
fn a_failure_a_person_meets_never_shows_them_the_plumbing() {
    let root = repo_root();

    // The client must not report a failed request as a decoding problem.
    let client = std::fs::read_to_string(root.join("src/client.rs")).expect("client.rs");
    let code: String = client
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let squashed: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        squashed.contains("!status.is_success()"),
        "`post_json` decodes every body as JSON, so a plain-text refusal reaches the person as a \
         serde error beside an internal route"
    );

    // And the two messages name a command rather than only a problem.
    let batch = std::fs::read_to_string(root.join("src/cli/batch.rs")).expect("cli/batch.rs");
    assert!(
        batch.contains("devplane ls groups by project"),
        "the fan-out's unknown-project error does not say how to find the names"
    );
    let api = std::fs::read_to_string(root.join("src/api.rs")).expect("api.rs");
    assert!(
        api.contains("devplane asks lists every question"),
        "answering an unknown ask does not say how to find the real ones"
    );
}

/// **The inbox row says where it came from and how long it has waited — on
/// both surfaces.**
///
/// `devplane inbox` is a flat list across every project whose stated order is
/// *level, then age*. It printed the level, the title and the kind: no project,
/// so *one list, every project* was unreadable with two projects in it; and no
/// age, so a question waiting nineteen hours and one raised eight minutes ago
/// were the same row. **The board carried both all along**, which is the tell —
/// two surfaces over one payload and only one of them reading it.
///
/// Asserted over the types rather than over a rendering, because the defect was
/// a field the CLI's own struct did not have: the wire carried it and the
/// reader dropped it.
#[test]
fn both_inbox_surfaces_read_the_project_and_the_wait() {
    let root = repo_root();

    // The CLI's own view of an item must carry them…
    let render = std::fs::read_to_string(root.join("src/render.rs")).expect("render.rs");
    let item = render
        .split("pub struct InboxItem")
        .nth(1)
        .expect("InboxItem")
        .split("\n}")
        .next()
        .expect("the struct body");
    for field in ["project_name", "since"] {
        assert!(
            item.contains(&format!("pub {field}:")),
            "`InboxItem` has no `{field}`, so `devplane inbox` cannot say {}",
            if field == "since" {
                "how long a row has waited"
            } else {
                "which project a row is from"
            }
        );
    }

    // …and the row must actually print them. A field on a struct nothing reads
    // is the shape of the defect one layer along.
    let cli = std::fs::read_to_string(root.join("src/cli/inbox.rs")).expect("cli/inbox.rs");
    let code: String = cli
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    // Whitespace-insensitive: rustfmt breaks a method chain across lines, so
    // `i.since` is not a substring of the file even where the code reads it.
    let squashed: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        squashed.contains("i.project_name"),
        "the CLI inbox row never reads `project_name`"
    );
    assert!(
        squashed.contains("i.since") && squashed.contains("ago("),
        "the CLI inbox row never turns `since` into a duration"
    );

    // And the board's row, which is where both came from.
    let board = std::fs::read_to_string(root.join("ui/src/surfaces/inbox/Inbox.svelte"))
        .expect("Inbox.svelte");
    let board_squashed: String = board.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        board_squashed.contains("i.since") && board_squashed.contains("project_name"),
        "the board's inbox row stopped carrying what the CLI's was fixed to match"
    );
}

/// **The help screen lists the commands once, and it is the grouped list.**
///
/// The specification asks that the commands be *presented* under five names.
/// What shipped was clap's flat list of thirty-five **and then** the same
/// thirty-five in groups, because `after_help` renders below the listing it was
/// written to replace — the problem this feature exists to fix, twice, on one
/// screen. The doc comment on `groups_block` even said *"printed above clap's
/// own listing"*.
///
/// **Nothing caught it because every test asked whether the groups were
/// present.** A screen that carries both answers yes. So this asserts the shape
/// of the whole screen: each command appears once, and it appears indented
/// under a group heading rather than in a flat block.
#[test]
fn the_help_screen_lists_every_command_exactly_once() {
    use clap::CommandFactory;
    let help = devplane::cli::Cli::command().render_help().to_string();

    for (_, commands) in devplane::cli::COMMAND_GROUPS {
        for c in *commands {
            // A line that begins a listing entry for this command: leading
            // whitespace, the name, then whitespace or end of line. `show`
            // must not match `show` inside a description.
            let entries = help
                .lines()
                .filter(|l| {
                    let t = l.trim_start();
                    l.starts_with(' ')
                        && t.starts_with(c)
                        && t[c.len()..].chars().next().is_none_or(char::is_whitespace)
                })
                .count();
            assert_eq!(
                entries, 1,
                "`{c}` is listed {entries} times on the help screen. One grouped listing is the \
                 feature; clap's flat `{{subcommands}}` beside it is the problem twice. The help \
                 template must not contain `{{subcommands}}` or `{{all-args}}`."
            );
        }
    }

    // And the grouping is what a reader sees: every group heading is on it.
    for (name, _) in devplane::cli::COMMAND_GROUPS {
        assert!(
            help.contains(name),
            "the help screen does not carry the group `{name}`"
        );
    }

    // Each command carries its own description, read from clap rather than
    // written a second time — so the listing and `help <command>` agree.
    assert!(
        help.contains("Show what needs a human, most urgent first"),
        "the grouped listing dropped the descriptions, which is most of what a listing is for"
    );
}

/// This reads `COMMAND_GROUPS` — the one place the grouping is decided — and
/// holds the page to it, group by group and command by command. The page may
/// word a heading however it likes as long as it is the heading the code uses;
/// what it may not do is put a command in a group the binary does not.
#[test]
fn the_site_and_the_binary_agree_which_group_a_command_is_in() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let page =
        std::fs::read_to_string(root.join("site/content/docs/cli.md")).expect("the CLI reference");

    // The table rows, as the page writes them:
    //   | **Group name** | `a` `b` `c` |
    let rows: Vec<(String, Vec<String>)> = page
        .lines()
        .filter(|l| l.starts_with("| **"))
        .filter_map(|l| {
            let mut cells = l.trim_matches('|').split('|');
            let name = cells.next()?.trim().trim_matches('*').trim().to_string();
            let commands = cells
                .next()?
                .split('`')
                .map(str::trim)
                .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase()))
                .map(str::to_string)
                .collect::<Vec<_>>();
            (!commands.is_empty()).then_some((name, commands))
        })
        .collect();

    assert_eq!(
        rows.len(),
        devplane::cli::COMMAND_GROUPS.len(),
        "the page lists {} groups and the binary has {} — found: {:?}",
        rows.len(),
        devplane::cli::COMMAND_GROUPS.len(),
        rows.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    for (name, commands) in devplane::cli::COMMAND_GROUPS {
        let (_, on_page) = rows
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("the page has no group called {name:?}"));

        let want: Vec<&str> = commands.to_vec();
        let got: Vec<&str> = on_page.iter().map(String::as_str).collect();
        assert_eq!(
            got, want,
            "group {name:?} differs between the page and the binary.\n\
             page:   {got:?}\n\
             binary: {want:?}\n\
             The grouping is decided in `COMMAND_GROUPS`; the page is a copy of it."
        );
    }
}

/// **The README's command count is the binary's.**
///
/// This number has drifted twice. A pass counted a `--help` listing by eye,
/// wrote **thirty-six** into two files, and found the error only by piping the
/// listing through `wc`; the reverse happened when a thirty-sixth command was
/// added and the prose stayed at thirty-five.
///
/// Counting by eye has now failed in both directions, so it is counted here.
/// The number in the prose is the length of `COMMAND_GROUPS`, which is also
/// what the grouped block prints — `help` is listed by clap and belongs to no
/// errand, so it is not among them.
#[test]
fn the_readme_names_as_many_commands_as_the_binary_groups() {
    const WORDS: &[(&str, usize)] = &[
        ("thirty-three", 33),
        ("thirty-four", 34),
        ("thirty-five", 35),
        ("thirty-six", 36),
        ("thirty-seven", 37),
    ];

    let grouped: usize = devplane::cli::COMMAND_GROUPS
        .iter()
        .map(|(_, cmds)| cmds.len())
        .sum();

    let readme = std::fs::read_to_string("README.md").expect("README.md");
    let found: Vec<(&str, usize)> = WORDS
        .iter()
        .filter(|(w, _)| readme.contains(&format!("{w} commands")))
        .copied()
        .collect();

    assert_eq!(
        found.len(),
        1,
        "the README should name the command count exactly once; it names {found:?}"
    );
    let (word, n) = found[0];
    assert_eq!(
        n, grouped,
        "the README says `{word} commands` and `COMMAND_GROUPS` holds {grouped}. \
         The grouped block is what a reader sees, and `help` is in no errand."
    );
}

/// **The published package carries the interface, and nothing it cannot use.**
///
/// `build.rs` embeds `ui/dist` and reads nothing else. Two ways to get that
/// wrong, and both are silent:
///
/// *Leaving the bundle out* ships a binary whose one page says it was built
/// without an interface — the failure `build.rs`'s own comment names, and which
/// the release workflow was already fixed for once.
///
/// *Putting the sources in* shipped 324 KB nobody could use: `ui/src` was
/// packaged while `package.json`, `vite.config.ts`, `tsconfig.json` and
/// `index.html` were not, so nothing in the package could rebuild the bundle
/// from them. Half a build input is not a build input.
#[test]
fn the_package_carries_the_built_interface_and_not_its_sources() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    let include = manifest
        .split("include = [")
        .nth(1)
        .and_then(|r| r.split(']').next())
        .expect("an include list");

    // What `build.rs` actually reads.
    for want in ["/ui/dist/*.html", "/ui/dist/*.js", "/ui/dist/*.css"] {
        assert!(
            include.contains(want),
            "`{want}` is not packaged, so a crates.io install builds a binary with no \
             interface — which it reports on the one page a person would open, after they \
             have installed it."
        );
    }

    // A source map is four times the bundle and nothing loads it.
    assert!(
        !include.contains("/ui/dist/*.map"),
        "the source map is packaged: four times the bundle, loaded by nothing"
    );

    // And the sources, which the package cannot build.
    let build_rs = std::fs::read_to_string("build.rs").expect("build.rs");
    assert!(
        !build_rs.contains("ui/src"),
        "`build.rs` reads `ui/src` now, so this guard's premise is wrong — \
         either package the whole UI project or stop reading its sources"
    );
    assert!(
        !include.contains("\"/ui/src\""),
        "`ui/src` is packaged and nothing in the package can build it: the build files \
         (`package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`) are not included, \
         and `build.rs` embeds `ui/dist` rather than compiling anything."
    );
}

/// **The published docs say what is true, never what changed.**
///
/// A changelog records movement; a page records state. Defect history in a
/// reference is dead weight to the one reader it has — somebody trying to use
/// the thing — and it goes stale in a way nothing checks, because a sentence
/// about the past is never wrong about the present.
///
/// Three separate passes put it back (a nav rename explained as a correction, a
/// surface described by what it used to be, an escalation justified by the bug
/// that preceded it), so this is counted rather than remembered.
///
/// The phrases are the unambiguous ones. Ordinary past tense about runtime — *what
/// it was waiting on*, *before it was killed* — is prose, not history, and stays.
#[test]
fn the_published_docs_are_not_a_changelog() {
    const HISTORY: &[&str] = &[
        "used to",
        "before this,",
        "previously,",
        "was renamed",
        "was added in",
        "has since been",
        "stopped being",
        "this release",
        "in an earlier version",
        "it was the other way",
    ];

    let root = repo_root();
    let mut pages: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("site/content/docs"))
        .expect("the docs")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    pages.push(root.join("README.md"));
    pages.sort();
    assert!(
        pages.len() > 10,
        "the docs directory has stopped being read"
    );

    let mut found = Vec::new();
    for page in &pages {
        let text = std::fs::read_to_string(page).unwrap_or_default();
        let mut fenced = false;
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            // A fenced block is sample output, and a table row is a value.
            if fenced || line.trim_start().starts_with('|') {
                continue;
            }
            let lower = line.to_lowercase();
            for phrase in HISTORY {
                if lower.contains(phrase) {
                    found.push(format!(
                        "{}:{} — \"{}\" in: {}",
                        page.file_name().unwrap_or_default().to_string_lossy(),
                        n + 1,
                        phrase,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "the published docs describe changes rather than state. A page says what is true now; \
         the changelog says what moved:\n  {}",
        found.join("\n  ")
    );
}

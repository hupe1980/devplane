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
        ("help", "clap's builtin"),
    ];

    let help = String::from_utf8(
        std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("--help")
            .output()
            .expect("the binary runs")
            .stdout,
    )
    .expect("help is text");

    let commands: Vec<String> = help
        .lines()
        .skip_while(|l| !l.starts_with("Commands:"))
        .filter_map(|l| {
            let rest = l.strip_prefix("  ")?;
            let word = rest.split_whitespace().next()?;
            match rest.starts_with(char::is_alphabetic) {
                true => Some(word.to_string()),
                false => None,
            }
        })
        .collect();
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

    // And the exemptions are real commands, so the list cannot rot into a set of
    // names for things that no longer exist.
    for (name, why) in UNDOCUMENTED {
        assert!(
            commands.iter().any(|c| c == name),
            "`{name}` is exempted from the reference ({why}) but is not a command"
        );
    }
}

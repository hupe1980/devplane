//! Documentation examples and the parser must agree.
//!
//! `devplane.toml` uses `deny_unknown_fields`, so one bad key in a documented example stops
//! the whole file loading. Every example must parse, mean something, and pass `devplane check`.

use std::path::{Path, PathBuf};

/// One fenced ```toml block, with whatever file its first comment names.
struct Block {
    /// Where it came from, for the failure message.
    source: String,
    /// The file this is an example of, from its leading comment; empty means `devplane.toml`.
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
            // Named by its comment, else by shape: `[agents.…]` means `agents.toml`.
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
            // `reference/` is fetched third-party documentation; its `toml` blocks are not ours.
            // An `.archive-*` directory is unmaintained by declaration, and its examples
            // describe configuration that has since been deleted.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "reference" || name.starts_with(".archive-") {
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
        // A fragment illustrates one key in context; it must still be valid TOML.
        if b.names.contains("fragment") {
            assert!(
                toml::from_str::<toml::Value>(&b.body).is_ok(),
                "{}: this fragment is not valid TOML:\n{}",
                b.source,
                b.body
            );
            continue;
        }

        if b.names.contains("app.toml") {
            // Parsed as the app parses it; an example left on defaults is wrong.
            let dir = std::env::temp_dir().join(format!(
                "vp-doc-app-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("app.toml");
            std::fs::write(&path, &b.body).unwrap();
            let (_, problems) = devplane::config::app_config_from(&path);
            assert!(
                problems.is_empty(),
                "{}: this `app.toml` example does not parse: {problems:?}\n{}",
                b.source,
                b.body
            );
            std::fs::remove_dir_all(&dir).ok();
            continue;
        }

        if b.names.contains("agents.toml") {
            // It must add an agent, pinned.
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

        // A block that parses because every key was ignored teaches nothing.
        assert_ne!(
            config,
            devplane::core::ProjectConfig::default(),
            "{}: parses to nothing at all:\n{}",
            b.source,
            b.body
        );

        // And `devplane check` must accept it.
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
    // README.md ships to crates.io.
    const README: &str = include_str!("../README.md");
    let configs = check(blocks_in("README.md", README));
    assert!(configs >= 1, "the README shows no devplane.toml at all");
}

#[test]
fn every_example_on_the_documentation_site_is_one_that_works() {
    // The configuration reference lives here, so it is the copy most likely to drift.
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

/// Reports a skipped test; under `CI`, fails instead.
///
/// The ignored tests below read gitignored inputs; `just notes` runs them where present.
fn skip(why: &str) {
    if std::env::var("CI").is_ok() {
        panic!("{why} — a skip cannot pass in CI");
    }
    eprintln!("skipped: {why}");
}

#[test]
#[ignore = "reads the gitignored concepts/; `just notes` runs it where the notes are present"]
fn every_example_in_the_internal_notes_is_one_that_works() {
    let dir = repo_root().join("concepts");
    if !dir.is_dir() {
        skip("concepts/ is not in this checkout, so no example in the notes was checked");
        return;
    }
    check(blocks_under(&dir));
}

/// Every key a project can set appears in the configuration reference.
///
/// A key that is read but undocumented is a feature nobody can find.
#[test]
fn every_key_a_project_can_set_is_in_the_reference() {
    // Keys come from the source, not a serialised default (TOML omits `None`). Structs that
    // derive `Deserialize` are writable; `Serialize`-only ones are output.
    let src =
        std::fs::read_to_string(repo_root().join("src/core/config.rs")).expect("the config module");

    let mut keys: Vec<String> = Vec::new();
    for block in src.split("pub struct ").skip(1) {
        let Some(header_end) = block.find('{') else {
            continue;
        };
        // The derive list is the tail of the previous block.
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

    // The README shows a subset on purpose, so it is not a reference.
    let sources: Vec<String> = ["site/content/docs/configuration.md"]
        .iter()
        .filter_map(|p| std::fs::read_to_string(repo_root().join(p)).ok())
        .collect();
    assert!(!sources.is_empty(), "no configuration reference found");

    // A mention is the key in backticks or as a TOML assignment, not a bare substring:
    // `only`, `max` or `file` would match ordinary English.
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

/// The release workflow and CI both build every target in `[workspace.metadata.dist]`:
/// a missing one would silently drop out of the release, or first be built at a tag.
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

    // CI builds the same targets on every push, so a tag is never the first
    // build of one.
    let ci = std::fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("ci.yml");
    let mut ci_built: Vec<String> = ci
        .lines()
        .filter_map(|l| l.trim().strip_prefix("target: "))
        .map(|t| t.trim().to_string())
        .collect();
    ci_built.sort();
    ci_built.dedup();
    assert_eq!(
        declared_sorted, ci_built,
        "[workspace.metadata.dist] targets and the ci.yml build matrix disagree"
    );
}

/// The Spec Kit version this integration was read against is recorded as a version.
#[test]
fn the_spec_kit_version_this_was_read_against_is_recorded() {
    use devplane::spec::SPEC_KIT_READ_AGAINST;

    assert!(
        SPEC_KIT_READ_AGAINST
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
        "the recorded version must be a version: {SPEC_KIT_READ_AGAINST}"
    );

    // The comparison with the installed copy lives in `scripts/concepts-check.sh`: Spec
    // Kit's directory is gitignored and the published tree may not point at it.
}

/// The npm package is generated by `dist` from this manifest, so its version is the
/// crate's and it cannot phone home.
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

    // An updater checks a remote at run time; the docs promise no network calls.
    assert_eq!(
        dist.get("install-updater").and_then(toml::Value::as_bool),
        Some(false),
        "install-updater must be explicitly false: an updater checks a remote for a version, \
         which is the one network call the package promises not to make"
    );

    // No checked-in package.json may claim the published name.
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

    // The release workflow builds it with `dist`.
    let workflow = std::fs::read_to_string(repo_root().join(".github/workflows/release.yml"))
        .expect("release.yml");
    assert!(
        workflow.contains("dist build --artifacts=global"),
        "the npm package must be built by `dist` from the manifest, or its version is not the \
         crate's by construction"
    );
}

/// Every rule the permissions page lists as refused is reported by the gate, and every
/// rule it teaches as ordinary is clean.
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
            // Every row is a deny-side rule.
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

/// Every published page that names the gate baseline names `SYNTAX_MODELLED_ON`.
#[test]
fn every_page_that_names_the_gate_baseline_names_the_one_the_binary_holds() {
    let baseline = devplane::core::policy::SYNTAX_MODELLED_ON;
    let root = repo_root();
    // Phrasings stating the harness baseline. "Built against Claude Code X" is a different
    // fact (the development release) and deliberately not checked here.
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

/// `llms.txt` names every command in `enum Command`.
#[test]
fn the_agent_facing_index_names_every_command() {
    let llms = include_str!("../site/static/llms.txt");
    let cli = include_str!("../src/cli/mod.rs");

    // The enum body only, so fields or match arms elsewhere are not taken for commands.
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
        // clap's default rename: `SpecKit` -> `spec-kit`, `Ls` -> `ls`.
        let mut kebab = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                kebab.push('-');
            }
            kebab.extend(c.to_lowercase());
        }
        // Hook-invoked commands (`hook`, `statusline`) are listed too, marked as such.
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

/// The plugin manifests match the crate version and use no vendor-reserved name, which
/// would make the marketplace unloadable.
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

    // Vendor-reserved marketplace names, matched case-insensitively.
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

    // The shipped MCP server must be this binary's read-only surface.
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

/// Every plugin skill has valid frontmatter (a broken one is silently absent) and names
/// only commands this binary has.
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

        // The loader silently skips a file without a closed frontmatter block.
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

        // The description is all an agent sees; it must say when to use the skill.
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

        // And the commands it names exist.
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

/// Spec Kit's hook contract still contains the clauses this integration relies on.
///
/// A reworded clause could leave a registered hook that never fires.
#[test]
#[ignore = "reads the gitignored .claude/skills/; `just notes` runs it where Spec Kit is installed"]
fn the_speckit_hook_contract_still_says_what_this_feature_relies_on() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let skill = root.join(".claude/skills/speckit-implement/SKILL.md");
    let Ok(body) = std::fs::read_to_string(&skill) else {
        skip("Spec Kit is not installed here, so its hook contract was not checked");
        return;
    };

    // Quoted verbatim: a paraphrase would survive the rewording that matters.
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
         {skill:?} and `devplane speckit` before trusting it:\n  {}",
        gone.join("\n  ")
    );

    // And the event we default to is one the installed copy actually reads.
    assert!(
        body.contains(&format!("hooks.{}", devplane::spec::DEFAULT_HOOK_EVENT)),
        "`{}` is not an event this Spec Kit version looks for",
        devplane::spec::DEFAULT_HOOK_EVENT
    );
}

/// Every session status, wait reason and state the vendor documents is one this build
/// handles, so a new vendor value fails here rather than being mishandled.
#[test]
#[ignore = "reads the gitignored concepts/reference/; `just notes` runs it where the corpus is fetched"]
fn every_session_state_the_vendor_documents_is_one_this_build_handles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let Ok(reference) =
        std::fs::read_to_string(root.join("concepts/reference/claude-code/agent-view.md"))
    else {
        skip("concepts/reference/ is not fetched, so the vendor's session states were not checked");
        return;
    };

    // The `status` row of the vendor's field table.
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
    // Counted, so a value nobody here has thought about is noticed.
    let backticked = status_row.matches('`').count() / 2;
    assert_eq!(
        backticked, 5,
        "`pid`, `status` and three values is five quoted terms; the row now has \
         {backticked}, so something was added or removed: {status_row}"
    );

    // Every documented wait reason must reach a person.
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

    // The background `state` vocabulary, from its own row.
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

    // The sentence the inbox's central inference rests on.
    assert!(
        reference.contains("`blocked` always means the session needs something from you"),
        "the vendor no longer says `blocked` always means a person is needed, \
         so the inbox's central inference needs re-reading"
    );
}

/// Every command, hidden ones included, is in the CLI reference or exempted here with a
/// reason. Hidden commands like `mcp` are still configured by people.
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

    // Hidden from `--help` as machine surfaces, but still owed a reference entry.
    const HIDDEN: &[&str] = &["mcp", "statusline", "hook"];

    let help = String::from_utf8(
        std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("--help")
            .output()
            .expect("the binary runs")
            .stdout,
    )
    .expect("help is text");

    // The command list comes from clap, not from the rendered help.
    let commands: Vec<String> = {
        use clap::CommandFactory;
        // `build()` first, so clap's generated `help` subcommand exists.
        let mut cmd = devplane::cli::Cli::command();
        cmd.build();
        cmd.get_subcommands()
            .map(|s| s.get_name().to_string())
            .collect()
    };
    // What a person sees: command lines are four-space indented under group headings.
    // The vacuity assertions below catch a parse that stops finding them.
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

    // Hidden commands are checked by asking the binary: `--help` on a non-command fails.
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

    // The exemptions must be real commands.
    for (name, why) in UNDOCUMENTED {
        // `help` is clap's own, so both sets are consulted.
        let known = commands.iter().any(|c| c == name)
            || listed.iter().any(|c| c == name)
            || HIDDEN.contains(name);
        assert!(
            known,
            "`{name}` is exempted from the reference ({why}) but is not a command"
        );
    }
}

/// An error a person can act on names the way out, and never shows a serde error, an
/// internal route or the port.
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
    let preflight =
        std::fs::read_to_string(root.join("src/core/preflight.rs")).expect("core/preflight.rs");
    assert!(
        preflight.contains("devplane ls groups by project"),
        "the unknown-project refusal does not say how to find the names"
    );
    let api = std::fs::read_to_string(root.join("src/api.rs")).expect("api.rs");
    assert!(
        api.contains("devplane inbox --all lists every question"),
        "answering an unknown ask does not say how to find the real ones"
    );
}

/// Both inbox surfaces (CLI and board) show each row's project and how long it has waited.
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

    // …and the row must print them.
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

    // And the interface's inbox list.
    let board = std::fs::read_to_string(root.join("ui/src/surfaces/inbox/List.svelte"))
        .expect("inbox/List.svelte");
    let board_squashed: String = board.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        board_squashed.contains("i.since") && board_squashed.contains("project_name"),
        "the board's inbox row stopped carrying what the CLI's was fixed to match"
    );
}

/// The help screen lists each command exactly once, under a group heading.
#[test]
fn the_help_screen_lists_every_command_exactly_once() {
    use clap::CommandFactory;
    let tree = devplane::cli::Cli::command();
    let help = tree.clone().render_help().to_string();
    // A command this build hides (`app` without its feature) is listed nowhere.
    let hidden = |c: &str| {
        tree.get_subcommands()
            .any(|s| s.get_name() == c && s.is_hide_set())
    };

    for (_, commands) in devplane::cli::COMMAND_GROUPS {
        for c in commands.iter().filter(|c| !hidden(c)) {
            // A listing entry: indented, the name, then whitespace or end of line.
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

    // Descriptions come from clap, so the listing and `help <command>` agree.
    assert!(
        help.contains("Show what needs a human, most urgent first"),
        "the grouped listing dropped the descriptions, which is most of what a listing is for"
    );
}

/// The site's group table matches `COMMAND_GROUPS`, group by group and command by command.
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

/// The README's command count equals the commands in `COMMAND_GROUPS` (`help` is in none).
#[test]
fn the_readme_names_as_many_commands_as_the_binary_groups() {
    const WORDS: &[(&str, usize)] = &[
        ("twenty-eight", 28),
        ("twenty-nine", 29),
        ("thirty", 30),
        ("thirty-one", 31),
        ("thirty-two", 32),
        ("thirty-three", 33),
        ("thirty-four", 34),
        ("thirty-five", 35),
        ("thirty-six", 36),
        ("thirty-seven", 37),
        ("thirty-eight", 38),
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

/// The published package carries the built `ui/dist` bundle, and neither the source map
/// nor `ui/src`, which it could not build.
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

/// The published docs state what is true, never what changed.
///
/// Only unambiguous history phrases count; runtime past tense is ordinary prose.
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

/// Every route the MCP server asks for is one the host serves.
///
/// Routes are `format!` strings, so a deleted handler is not a compile error.
#[test]
fn every_route_the_mcp_server_asks_for_is_one_the_host_serves() {
    let mcp = std::fs::read_to_string("src/mcp.rs").expect("src/mcp.rs");
    let api = std::fs::read_to_string("src/api.rs").expect("src/api.rs");

    let mut asked: Vec<String> = Vec::new();
    for (i, _) in mcp.match_indices("\"/api/") {
        let rest = &mcp[i + 1..];
        let end = rest.find('"').expect("unterminated string literal");
        let path = &rest[..end];
        // Stop at the query string: the router only knows the path.
        let path = path.split('?').next().unwrap_or(path);
        if !asked.iter().any(|a| a == path) {
            asked.push(path.to_string());
        }
    }
    assert!(
        !asked.is_empty(),
        "no routes found in src/mcp.rs — this test has stopped reading anything"
    );

    let mut missing = Vec::new();
    for path in &asked {
        // Literal match only: the MCP server uses no `{id}` routes.
        if !api.contains(&format!("\"{path}\"")) {
            missing.push(path.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "the MCP server asks for {} the host does not serve: {missing:?}\n\
         These are composed with `format!`, so nothing else will tell you.",
        if missing.len() == 1 {
            "a route"
        } else {
            "routes"
        }
    );
}

/// Every visible command is in `COMMAND_GROUPS`, the only listing `--help` prints; the
/// converse of `the_help_screen_lists_every_command_exactly_once`.
#[test]
fn every_command_the_binary_offers_is_in_a_group() {
    use clap::CommandFactory;
    let cmd = devplane::cli::Cli::command();
    let grouped: std::collections::BTreeSet<&str> = devplane::cli::COMMAND_GROUPS
        .iter()
        .flat_map(|(_, cs)| cs.iter().copied())
        .collect();

    let missing: Vec<&str> = cmd
        .get_subcommands()
        // Hidden commands (`mcp`) are hidden from every listing on purpose.
        .filter(|c| !c.is_hide_set())
        .map(|c| c.get_name())
        .filter(|n| *n != "help" && !grouped.contains(n))
        .collect();

    assert!(
        missing.is_empty(),
        "these commands exist and are in no group, so `devplane --help` does not \
         list them: {missing:?}"
    );
    assert!(
        grouped.len() > 20,
        "only {} commands are grouped; this guard has stopped reading the tree",
        grouped.len()
    );
}

/// Every key the reference's tables document is one the parser accepts; with
/// `deny_unknown_fields`, a stale row fails the whole file for anyone who copies it.
#[test]
fn every_key_the_reference_documents_is_one_a_project_can_set() {
    let doc = std::fs::read_to_string(repo_root().join("site/content/docs/configuration.md"))
        .expect("the configuration reference");
    let src =
        std::fs::read_to_string(repo_root().join("src/core/config.rs")).expect("the config module");

    let mut documented: Vec<&str> = Vec::new();
    for line in doc.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let Some((cell, _)) = rest.split_once('`') else {
            continue;
        };
        // A nested key is written by its path; the field is the last segment.
        let key = cell.rsplit('.').next().unwrap_or(cell);
        if !key.is_empty() && key.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            documented.push(key);
        }
    }
    documented.sort_unstable();
    documented.dedup();
    assert!(
        documented.len() > 20,
        "the reference's tables stopped parsing as tables: {documented:?}"
    );

    // A field, or a `serde` rename.
    let settable = |k: &str| {
        src.lines().any(|l| {
            let l = l.trim();
            l.strip_prefix("pub ")
                .and_then(|r| r.split_once(':'))
                .is_some_and(|(f, _)| f == k)
        }) || src.contains(&format!("rename = \"{k}\""))
    };
    let gone: Vec<&&str> = documented.iter().filter(|k| !settable(k)).collect();
    assert!(
        gone.is_empty(),
        "the reference documents these and the parser rejects them, \
         which fails the whole file for anyone who copies a line: {gone:?}"
    );
}

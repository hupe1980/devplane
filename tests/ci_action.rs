//! Lints on the verify action and the CI job that runs it: text checks that a
//! pull request cannot rewrite what judges it, and that the action installs a
//! pinned, checksummed release and reads its verdict as JSON. File reads only.

use std::path::PathBuf;

fn read(path: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Lines that are not YAML comments, trimmed.
fn code_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

/// The directory the CI job copies the merge base's action.yml into.
const BASE_ACTION: &str = "./.devplane-base-action";

/// A local `uses:` resolves in the pull request's own checkout; the only one
/// allowed is the directory holding the merge base's action.yml, filled from
/// the merge-base worktree the binary is built in.
#[test]
fn ci_runs_the_bases_action_never_the_pull_requests() {
    let ci = read(".github/workflows/ci.yml");
    let local: Vec<&str> = code_lines(&ci)
        .into_iter()
        .filter_map(|l| l.trim_start_matches("- ").strip_prefix("uses:"))
        .map(|u| u.trim().trim_matches(['"', '\'']))
        .filter(|u| u.starts_with('.'))
        .collect();
    assert!(!local.is_empty(), "ci.yml no longer runs the verify action");
    for u in &local {
        assert_eq!(
            u.trim_end_matches('/'),
            BASE_ACTION,
            "ci.yml runs a local action from the pull request's own checkout: `uses: {u}`"
        );
    }
    let filled = code_lines(&ci).into_iter().any(|l| {
        l.starts_with("cp ")
            && l.contains("\"$RUNNER_TEMP/devplane-base/action.yml\"")
            && l.contains(&BASE_ACTION[2..])
    });
    assert!(
        filled,
        "{BASE_ACTION} is not filled from the merge base's worktree"
    );
    assert!(
        ci.contains("git worktree add --detach \"$RUNNER_TEMP/devplane-base\" \"$from\""),
        "the merge-base worktree the action is copied from is gone"
    );
}

/// The `default:` of one input in action.yml.
fn input_default<'a>(action: &'a str, input: &str) -> Option<&'a str> {
    let mut lines = action.lines().skip_while(|l| *l != "inputs:");
    let header = format!("  {input}:");
    lines.find(|l| *l == header)?;
    lines
        .take_while(|l| l.starts_with("    ") || l.trim().is_empty())
        .find_map(|l| l.trim().strip_prefix("default:"))
        .map(|d| d.trim().trim_matches(['"', '\'']))
}

#[test]
fn the_action_installs_the_release_it_shipped_with_by_default() {
    let action = read("action.yml");
    let manifest: toml::Value = toml::from_str(&read("Cargo.toml")).expect("Cargo.toml parses");
    let crate_version = manifest["package"]["version"].as_str().expect("a version");
    let default = input_default(&action, "version").expect("a default version");
    assert_ne!(default, "latest", "the default version is unpinned");
    assert_eq!(
        default,
        format!("v{crate_version}"),
        "action.yml's default version is not the crate's"
    );
}

#[test]
fn the_action_checks_the_archive_and_pipes_nothing_into_a_shell() {
    let action = read("action.yml");
    for l in code_lines(&action) {
        // The command after each `|`: a shell there runs whatever was downloaded.
        for piped in l.split('|').skip(1) {
            let cmd = piped.split_whitespace().next().unwrap_or("");
            let cmd = cmd.rsplit('/').next().unwrap_or(cmd);
            assert!(
                !["sh", "bash", "zsh", "dash"].contains(&cmd),
                "action.yml pipes into a shell: {l}"
            );
        }
        assert!(
            !l.contains("installer.sh"),
            "action.yml runs an installer script: {l}"
        );
    }
    assert!(
        action.contains("sha256sum -c") && action.contains("shasum -a 256 -c"),
        "action.yml does not check the archive's SHA-256 on Linux and macOS"
    );
    let check = action.find("sha256sum -c").unwrap();
    let unpack = action
        .find("tar -x")
        .expect("action.yml unpacks an archive");
    assert!(check < unpack, "action.yml unpacks before it checks");
}

#[test]
fn the_action_reads_the_verdict_as_json() {
    let action = read("action.yml");
    for l in code_lines(&action) {
        let words: Vec<&str> = l
            .split(|c: char| c.is_whitespace() || c == '(' || c == '|')
            .collect();
        let text_tool = ["sed", "grep", "awk", "cut"]
            .iter()
            .any(|t| words.contains(t));
        let reads_json = ["$out", ".json", "verdict", "statement"]
            .iter()
            .any(|j| l.contains(j));
        assert!(
            !(text_tool && reads_json),
            "action.yml reads the JSON document with a text tool: {l}"
        );
    }
    assert!(
        action.contains("jq -r '.verdict"),
        "action.yml does not read `.verdict` with jq"
    );
}

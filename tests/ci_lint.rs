//! Lints on the CI and release workflows: text checks that the pipelines build
//! and publish what the product claims. File reads only, so this binary is fast.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

/// CI and release workflows build the interface, and so does every job that
/// runs `cargo publish`; `build.rs` tolerates a missing bundle, so nothing
/// else would catch it.
#[test]
fn the_release_pipeline_builds_the_interface() {
    let root = repo_root();
    for wf in ["ci.yml", "release.yml"] {
        let path = root.join(".github/workflows").join(wf);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        assert!(
            text.contains("npm run build"),
            "{wf} never builds the interface. A missing bundle is not a build failure — it is a \
             binary that serves nothing, and nobody finds out until they install it."
        );
        assert!(
            text.contains("setup-node"),
            "{wf} runs `npm run build` with no node installed"
        );
    }

    // Per job, not per file: each publishing job needs its own build step.
    let text =
        std::fs::read_to_string(root.join(".github/workflows/release.yml")).expect("release.yml");

    // A job starts at exactly two spaces of indent; its steps are deeper.
    let mut jobs: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let head = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim().contains(' ');
        if head {
            jobs.push((line.trim().trim_end_matches(':').to_string(), String::new()));
        } else if let Some(last) = jobs.last_mut() {
            last.1.push_str(line);
            last.1.push('\n');
        }
    }
    assert!(
        jobs.iter().any(|(_, b)| b.contains("cargo publish")),
        "no job in release.yml runs `cargo publish`, so this check has stopped matching the file"
    );

    for (name, job) in jobs.iter().filter(|(_, b)| b.contains("cargo publish")) {
        assert!(
            job.contains("npm run build"),
            "the `{name}` job runs `cargo publish` without building the interface first. \
             `ui/dist/` is gitignored, so a clean checkout has none and `include` matches \
             nothing — the crate publishes and installs with no interface at all."
        );
    }
}

/// The packaged bundle is committed, release never uses `--allow-dirty`, and CI
/// checks the committed bundle matches its sources, so what ships is in a commit.
#[test]
fn the_published_bundle_is_accountable_to_a_commit() {
    let root = repo_root();

    let ignore = std::fs::read_to_string(root.join(".gitignore")).expect(".gitignore");
    assert!(
        !ignore
            .lines()
            .any(|l| l.trim() == "ui/dist/" || l.trim() == "ui/dist"),
        "`ui/dist/` is ignored wholesale, so the files `Cargo.toml` packages are not in git. \
         `cargo publish` then refuses, and the flag that silences it publishes bytes no commit \
         contains."
    );
    assert!(
        ignore.contains("ui/dist/*.map"),
        "the source map is not ignored: four times the bundle, packaged by nothing and loaded \
         by nothing"
    );

    let wf =
        std::fs::read_to_string(root.join(".github/workflows/release.yml")).expect("release.yml");
    assert!(
        !wf.contains("--allow-dirty"),
        "the release publishes with `--allow-dirty`. Whatever that flag lets through is \
         content the crate carries and no commit contains — and this product's whole claim \
         is that a reviewer does not have to trust it."
    );
    assert!(
        wf.contains("git diff --exit-code -- ui/dist"),
        "nothing checks that the committed bundle is what the sources produce, so it can go \
         stale silently — which is the one cost of committing it"
    );
}

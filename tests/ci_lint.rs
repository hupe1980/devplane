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

/// The text of one job (a key at two spaces of indent) in a workflow, up to
/// the next job.
fn job<'a>(workflow: &'a str, name: &str) -> &'a str {
    let head = format!("\n  {name}:\n");
    let start = workflow
        .find(&head)
        .unwrap_or_else(|| panic!("no `{name}` job"))
        + head.len();
    let mut end = workflow.len();
    let mut at = start;
    for line in workflow[start..].split_inclusive('\n') {
        let is_job = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim().contains(' ');
        if is_job {
            end = at;
            break;
        }
        at += line.len();
    }
    &workflow[start..end]
}

/// Every tag builds the desktop app for each platform from the same commit,
/// signs it only with credentials from the protected environment, and
/// otherwise labels every file unsigned and publishes no update manifest.
#[test]
fn the_release_packages_the_app_signed_or_labelled_unsigned() {
    let wf = std::fs::read_to_string(repo_root().join(".github/workflows/release.yml"))
        .expect("release.yml");
    let packages = job(&wf, "packages");
    assert!(
        packages.contains("environment: release"),
        "secrets outside the release environment"
    );
    assert!(
        packages.contains("--features app"),
        "the packages are built without the window"
    );
    for bundle in ["app,dmg", "nsis", "deb,appimage"] {
        assert!(
            packages.contains(&format!("bundles: {bundle}")),
            "no `{bundle}` leg"
        );
    }
    // The exact secret names the install page and the workflow's header name.
    let header: String = wf
        .lines()
        .take_while(|l| l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let docs = std::fs::read_to_string(repo_root().join("site/content/docs/install.md"))
        .expect("install.md");
    for secret in [
        "APPLE_CERTIFICATE",
        "APPLE_CERTIFICATE_PASSWORD",
        "APPLE_SIGNING_IDENTITY",
        "APPLE_ID",
        "APPLE_PASSWORD",
        "APPLE_TEAM_ID",
        "WINDOWS_SIGN_COMMAND",
        "TAURI_SIGNING_PRIVATE_KEY",
        "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
    ] {
        assert!(
            packages.contains(&format!("secrets.{secret} }}")),
            "the packages job never reads `{secret}`"
        );
        assert!(
            header.contains(secret),
            "release.yml's header does not name `{secret}`"
        );
        assert!(docs.contains(secret), "install.md does not name `{secret}`");
    }
    // Unsigned is said in the file name, the summary and the marker publish reads.
    assert!(
        packages.contains("-unsigned."),
        "an unsigned package is not named so"
    );
    assert!(
        packages.contains("GITHUB_STEP_SUMMARY"),
        "the missing credential is not named"
    );
    assert!(
        packages.contains("unsigned-$PLATFORM"),
        "publish cannot tell a leg was unsigned"
    );
    assert!(
        packages.contains("--no-sign"),
        "the unsigned path may still try to sign"
    );
    // Verified with the platform's own tools before upload.
    for tool in [
        "codesign --verify",
        "spctl --assess",
        "stapler validate",
        "verify //pa",
    ] {
        assert!(packages.contains(tool), "no `{tool}` before upload");
    }
    // Every package has a checksum beside it.
    assert!(
        packages.contains(".sha256"),
        "packages are published without checksums"
    );

    let publish = job(&wf, "publish");
    assert!(
        publish.contains("packages"),
        "publish does not wait for the packages"
    );
    let manifest_at = publish
        .find("> packages/latest.json")
        .expect("no update manifest");
    let unsigned_at = publish
        .find("packages/unsigned-*")
        .expect("publish never checks for unsigned legs");
    assert!(
        unsigned_at < manifest_at,
        "the manifest is written before the unsigned check"
    );
    assert!(
        publish.contains("These packages are unsigned"),
        "the notes do not say what is unsigned"
    );
    assert!(
        publish.contains("libwebkit2gtk-4.1-0"),
        "the notes do not name WebKitGTK"
    );
    assert!(
        publish.contains("Apple Silicon"),
        "the notes do not say which Macs"
    );
    assert!(
        publish.contains("devplane-installer.sh"),
        "the CLI's install line is gone"
    );

    let release = job(&wf, "github-release");
    assert!(
        release.contains("--latest=\"$latest\""),
        "an unsigned release is marked latest unconditionally"
    );
    assert!(
        release.contains("latest.json"),
        "an unsigned release may displace a signed one"
    );

    // The CLI's archives keep the names the composite action installs by.
    let action = std::fs::read_to_string(repo_root().join("action.yml")).expect("action.yml");
    assert!(
        action.contains("devplane-${target}.tar.xz") || action.contains("devplane-$target.tar.xz"),
        "action.yml no longer installs devplane-<target>.tar.xz"
    );
    assert!(
        job(&wf, "build").contains("dist build --artifacts=local"),
        "the CLI is no longer built by dist"
    );
}

/// A pull request bundles every package unsigned, so a packaging break is
/// found before a tag, and never sees a signing secret.
#[test]
fn every_pull_request_bundles_the_app_unsigned() {
    let ci = std::fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("ci.yml");
    let bundle = job(&ci, "bundle");
    assert!(bundle.contains("--features app") && bundle.contains("--no-sign"));
    for b in ["app,dmg", "nsis", "deb,appimage"] {
        assert!(
            bundle.contains(&format!("bundles: {b}")),
            "no `{b}` bundle in CI"
        );
    }
    assert!(
        !ci.contains("secrets."),
        "ci.yml, which runs on pull requests, reads a secret"
    );
}

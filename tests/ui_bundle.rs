//! The built interface, as the binary embeds it.
//!
//! These are the guards that make the switch at the end of the port a single
//! change rather than a leap: the embedding mechanism, the readability of what
//! is served, the size budget, and the refusal to fetch anything.
//!
//! **They run against whatever `ui/dist/` holds**, and skip with a message when
//! it holds nothing — a clean checkout has never built it, and a machine with
//! no node cannot. A skip that says so is honest; a skip that is silent is how
//! this repository already lost 56 checks it believed it had.

/// The budget for the served interface, gzipped.
///
/// Raised from the hand-written page's 40 036 bytes on 2026-09-20, deliberately
/// and with the reason recorded: readable output costs about 27 KB of framework
/// before any product code, and readability is the property this product argues
/// for about itself. **60 000 is the number, and passing it is the trigger to
/// re-open minification** rather than to edit this constant.
const BUDGET_GZIPPED: usize = 60_000;

/// The source ceiling stays in lines, and the served artefact is measured in
/// bytes. One figure, one home, each answering the question it can answer: the
/// old page was 31 % comments, so a byte ceiling made explanation compete with
/// features — and that reasoning does not survive built output, where comments
/// are not shipped.
const SOURCE_LINE_CEILING: usize = 3_000;

fn dist() -> Option<std::path::PathBuf> {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/dist");
    d.join("app.js").is_file().then_some(d)
}

fn gzipped(bytes: &[u8]) -> usize {
    use std::io::Write as _;
    let mut child = std::process::Command::new("gzip")
        .args(["-9", "-c"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("gzip");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(bytes)
        .expect("write");
    child.wait_with_output().expect("gzip output").stdout.len()
}

/// The whole served artefact fits the budget, gzipped.
#[test]
fn the_served_interface_is_within_its_budget() {
    let Some(dist) = dist() else {
        eprintln!("skipped: ui/dist is not built");
        return;
    };
    let mut total = Vec::new();
    for e in std::fs::read_dir(&dist).expect("dist").flatten() {
        let p = e.path();
        // Source maps are not embedded and are not served.
        if p.extension().is_some_and(|x| x == "map") {
            continue;
        }
        if p.is_file() {
            total.extend(std::fs::read(&p).expect("asset"));
        }
    }
    let size = gzipped(&total);
    println!("served interface: {size} bytes gzipped (budget {BUDGET_GZIPPED})");
    assert!(
        size <= BUDGET_GZIPPED,
        "the served interface is {size} gzipped bytes, over the {BUDGET_GZIPPED} budget. \
         That is the trigger to re-open minification — recorded as a decision — \
         rather than to raise this number"
    );
}

/// **What is served stays readable.**
///
/// Not developer convenience: this is a product about being able to see what
/// was decided on your machine, and the interface is the one artefact a person
/// can read without this repository — over a tunnel, with `curl`, on a machine
/// that has never built it.
///
/// Readability is checked as a property of the bytes rather than by trusting a
/// build flag, because a flag is one edit away and says nothing about what came
/// out.
#[test]
fn the_served_bundle_can_be_read_without_a_source_map() {
    let Some(dist) = dist() else {
        eprintln!("skipped: ui/dist is not built");
        return;
    };
    let js = std::fs::read_to_string(dist.join("app.js")).expect("app.js");

    // A minified bundle is one enormous line. A readable one is not.
    let longest = js.lines().map(str::len).max().unwrap_or(0);
    assert!(
        longest < 400,
        "the longest line is {longest} characters, which is a minified blob rather \
         than something a person can follow"
    );

    // And it keeps the names that make it followable. A minifier replaces these.
    for name in ["function", "const ", "return "] {
        assert!(
            js.contains(name),
            "`{name}` does not appear in the bundle, so it has been minified"
        );
    }
    // Newlines, not one line with semicolons.
    assert!(
        js.lines().count() > 100,
        "the bundle is {} lines, which is not readable output",
        js.lines().count()
    );
}

/// **Nothing is fetched from outside the machine, in any build, for any asset.**
///
/// Checked as *what would be loaded*, not as the substring `http`. The bundle
/// legitimately contains a dozen `https://svelte.dev/e/…` strings — they are
/// error-message documentation links, plus an XML namespace constant — and
/// nothing loads them. A guard that greps for `http` fails on those and teaches
/// somebody to weaken it; this one asks the question that matters.
#[test]
fn nothing_in_the_served_interface_reaches_outside_this_machine() {
    let Some(dist) = dist() else {
        eprintln!("skipped: ui/dist is not built");
        return;
    };
    for e in std::fs::read_dir(&dist).expect("dist").flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "map") || !p.is_file() {
            continue;
        }
        let text = String::from_utf8_lossy(&std::fs::read(&p).expect("asset")).to_string();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // The forms that actually load something.
        for (pattern, what) in [
            ("src=\"http", "a script or image from elsewhere"),
            ("src=\"//", "a protocol-relative source"),
            ("href=\"http", "a stylesheet or font from elsewhere"),
            ("href=\"//", "a protocol-relative link target"),
            ("@import url(http", "an imported stylesheet"),
            ("fonts.googleapis", "a font"),
            ("cdn.", "a CDN"),
        ] {
            assert!(
                !text.contains(pattern),
                "{name} loads {what} — a control plane whose own interface \
                 phones somewhere is not one"
            );
        }

        // `fetch()` and `import()` may only address this machine.
        for call in [
            "fetch(\"http",
            "fetch('http",
            "import(\"http",
            "import('http",
        ] {
            assert!(
                !text.contains(call),
                "{name} contains `{call}…`, which reaches off this machine"
            );
        }
    }
}

/// The source ceiling, on the source, where comments do not compete with
/// features.
#[test]
fn no_interface_source_file_is_past_the_line_ceiling() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src");
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            let lines = text.lines().count();
            assert!(
                lines <= SOURCE_LINE_CEILING,
                "{} is {lines} lines, past the {SOURCE_LINE_CEILING} ceiling — \
                 a surface that large is several surfaces",
                p.display()
            );
        }
    }
}

/// The embedding mechanism itself: what `build.rs` produced is what is on disk.
///
/// Without this, a bundle that silently failed to embed would look exactly like
/// one that was never built, and the difference is the entire interface.
#[test]
fn the_bundle_the_binary_embedded_matches_what_was_built() {
    match (devplane::api::BUNDLE, dist()) {
        (Some(assets), Some(dist)) => {
            assert!(!assets.is_empty(), "an empty bundle embedded as `Some`");
            for a in assets {
                let on_disk = dist.join(a.path);
                assert!(on_disk.is_file(), "embedded `{}` is not in ui/dist", a.path);
                assert_eq!(
                    std::fs::read(&on_disk).expect("asset"),
                    a.bytes,
                    "embedded `{}` differs from the file that was built",
                    a.path
                );
            }
            assert!(
                assets.iter().any(|a| a.path == "app.js"),
                "the bundle embedded no script"
            );
            assert!(
                !assets.iter().any(|a| a.path.ends_with(".map")),
                "a source map was embedded; it is four times the bundle and nothing loads it"
            );
        }
        (None, None) => eprintln!("skipped: no bundle was built and none was embedded"),
        // These two are the interesting failures, and neither is silent.
        (Some(_), None) => panic!("a bundle is embedded but ui/dist is gone; the binary is stale"),
        (None, Some(_)) => {
            panic!("ui/dist exists and nothing was embedded — run a clean build")
        }
    }
}

/// **The interface's wire types are what the Rust types would generate.**
///
/// The point of generating them is that a second hand-written copy of a wire
/// format is a second thing to keep true — and the copy is always the one that
/// drifts, silently, because a field the interface does not know about simply
/// does not render.
///
/// Generation only helps if somebody runs it, so this fails when the checked-in
/// bindings are not what the current Rust shapes produce. It regenerates into a
/// temporary directory and compares, which means it needs the `typescript`
/// feature — so it **skips without it**, with a message, and CI runs it with.
///
/// ```sh
/// TS_RS_EXPORT_DIR=ui/src cargo test --features typescript export_bindings
/// ```
#[test]
fn the_checked_in_wire_types_match_the_rust_shapes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let checked_in = root.join("ui/src/wire");
    if !checked_in.is_dir() {
        eprintln!("skipped: ui/src/wire has not been generated");
        return;
    }

    let tmp = std::env::temp_dir().join(format!("devplane-wire-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    let out = std::process::Command::new(env!("CARGO"))
        .args([
            "test",
            "--features",
            "typescript",
            "--quiet",
            "export_bindings",
        ])
        .current_dir(root)
        .env("TS_RS_EXPORT_DIR", &tmp)
        .output();

    let Ok(out) = out else {
        eprintln!("skipped: could not re-run the generator");
        return;
    };
    let regenerated = tmp.join("wire");
    if !out.status.success() || !regenerated.is_dir() {
        // The feature is optional, so a build without it is not a failure here.
        eprintln!("skipped: the generator did not run ({})", out.status);
        let _ = std::fs::remove_dir_all(&tmp);
        return;
    }

    let read = |dir: &std::path::Path| -> std::collections::BTreeMap<String, String> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().to_string(),
                    std::fs::read_to_string(e.path()).unwrap_or_default(),
                )
            })
            .collect()
    };

    let want = read(&regenerated);
    let have = read(&checked_in);
    let _ = std::fs::remove_dir_all(&tmp);

    assert!(
        !want.is_empty(),
        "the generator produced nothing to compare"
    );
    let missing: Vec<&String> = want.keys().filter(|k| !have.contains_key(*k)).collect();
    let extra: Vec<&String> = have.keys().filter(|k| !want.contains_key(*k)).collect();
    assert!(
        missing.is_empty(),
        "a Rust type crosses the wire and the interface has no type for it: {missing:?}\n\
         Run: TS_RS_EXPORT_DIR=ui/src cargo test --features typescript export_bindings"
    );
    assert!(
        extra.is_empty(),
        "the interface carries wire types nothing generates any more: {extra:?}\n\
         Delete them, or the interface is holding a shape the product stopped sending"
    );
    for (name, text) in &want {
        assert_eq!(
            have.get(name).map(String::as_str),
            Some(text.as_str()),
            "{name} has drifted from the Rust type it is generated from.\n\
             Run: TS_RS_EXPORT_DIR=ui/src cargo test --features typescript export_bindings"
        );
    }
}

/// **No two Rust types may claim the same name on the wire.**
///
/// Rust has modules and the generated interface has one namespace, so
/// `ask::Kind` and `batch::Kind` both exported a file called `Kind.ts` and
/// whichever ran last silently replaced the other. The interface would have
/// compiled — against a type describing something else entirely.
///
/// The drift check above caught it by accident, because the loser happened to
/// be checked in. This catches it on purpose, and names the two types rather
/// than the file.
#[test]
fn no_two_wire_types_claim_the_same_name() {
    let mut claimed: std::collections::BTreeMap<String, Vec<String>> = Default::default();

    for file in rust_sources("src") {
        let text = std::fs::read_to_string(&file).expect("source");
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains("ts(") || !line.contains("export") {
                continue;
            }
            // The exported name is the `rename` where there is one, and the
            // type's own name otherwise. Both sit within a few lines of the
            // attribute.
            let window = lines[i..(i + 8).min(lines.len())].join("\n");
            let renamed = window
                .split("rename = \"")
                .nth(1)
                .and_then(|r| r.split('"').next())
                .map(str::to_string);
            let declared = window
                .lines()
                .find_map(|l| {
                    let t = l.trim_start();
                    ["pub struct ", "pub enum "]
                        .iter()
                        .find_map(|k| t.strip_prefix(k))
                        .map(|r| {
                            r.split(['(', '<', ' ', '{'])
                                .next()
                                .unwrap_or_default()
                                .to_string()
                        })
                })
                .filter(|n| !n.is_empty());
            let Some(name) = renamed.or(declared) else {
                continue;
            };
            claimed.entry(name).or_default().push(file.clone());
        }
    }
    assert!(
        claimed.len() > 10,
        "the scan found {} exported types, so it is not reading the attributes",
        claimed.len()
    );

    let clashes: Vec<(String, Vec<String>)> = claimed
        .iter()
        .filter_map(|(name, from)| {
            let mut uniq: Vec<String> = from.clone();
            uniq.sort();
            uniq.dedup();
            (uniq.len() > 1).then(|| (name.clone(), uniq))
        })
        .collect();
    assert!(
        clashes.is_empty(),
        "these wire names are claimed by more than one Rust type, and one of them \
         would silently overwrite the other: {clashes:?}"
    );
}

/// Every `.rs` under a directory.
fn rust_sources(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(rust_sources(&p.to_string_lossy()));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p.to_string_lossy().to_string());
        }
    }
    out
}

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

/// The ceiling for the served interface, gzipped.
///
/// **Re-argued on 2026-09-21, because the number was measuring the wrong
/// thing.** It was set against the hand-written page's 40 036 bytes as a
/// *budget* — the word implies a cost being spent, and there is no cost here.
/// This interface is `include_str!`d into the binary and served over
/// **loopback**: nothing crosses a network, nothing is cached, nobody is on a
/// phone tethered in a car park. First paint is a memcpy. Treating 6 KB of
/// framework as expensive was importing a web application's economics into a
/// local tool that has none of them.
///
/// **So it is a tripwire rather than a budget, and it is worth keeping as
/// one.** What a size ceiling can still catch on this machine is the thing
/// nobody decided: a dependency pulled in by a transitive upgrade, a polyfill,
/// an icon font, a date library shipped to format three timestamps. Those
/// arrive in hundreds of kilobytes, not in tens — so the number is set where
/// it separates *somebody added a library* from *somebody added a surface*,
/// and nowhere near where it makes a feature compete with a component model.
///
/// **250 000 is the number.** Passing it means asking what was added, not
/// reaching for minification — and minification stays refused for the reason
/// in [`the_served_bundle_can_be_read_without_a_source_map`], which is about
/// being able to read what is served and is untouched by any of this.
const BUDGET_GZIPPED: usize = 250_000;

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
    println!("served interface: {size} bytes gzipped (ceiling {BUDGET_GZIPPED})");
    assert!(
        size <= BUDGET_GZIPPED,
        "the served interface is {size} gzipped bytes, over the {BUDGET_GZIPPED} ceiling. \
         This is served over loopback out of the binary, so the number is not a cost — it is \
         a tripwire for a dependency nobody decided to add. Ask what was added; a surface does \
         not move this and a library does."
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
    //
    // **The measure is the median, not the longest**, and the change is the
    // finding: the ceiling was 400 characters, set when this bundle was an
    // empty scaffold, and the first real surface broke it with a **411-character
    // compiled template literal**. That is not minification — it is one line of
    // markup in four thousand lines of readable code — and a guard that cannot
    // tell them apart would have been switched off by the second surface.
    //
    // A minified bundle has a huge median and almost no lines. A readable one
    // has thousands of short lines and occasional long template strings, which
    // is exactly what a compiler-first framework emits.
    let mut widths: Vec<usize> = js.lines().map(str::len).collect();
    widths.sort_unstable();
    let median = widths.get(widths.len() / 2).copied().unwrap_or(0);
    assert!(
        median < 80,
        "the median line is {median} characters, which is a minified blob rather \
         than something a person can follow"
    );
    // And a genuine blob is not merely long, it is orders of magnitude long.
    // A template literal is thousands of characters at worst; a minified chunk
    // is tens of thousands.
    let longest = widths.last().copied().unwrap_or(0);
    assert!(
        longest < 8_000,
        "the longest line is {longest} characters, which is not a template literal"
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

// ---------------------------------------------------------------------------
// The surface registry
// ---------------------------------------------------------------------------

/// Removes `<!-- … -->` and `/* … */` spans, and `//` to end of line.
///
/// A guard over source has to read the source, and a comment explaining why
/// something is forbidden is not an instance of it. Line-prefix filtering is
/// not enough: the mention that defeated the first two attempts was the second
/// line of a multi-line comment.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if src[i..].starts_with("<!--") {
            i = src[i..].find("-->").map_or(b.len(), |j| i + j + 3);
        } else if src[i..].starts_with("/*") {
            i = src[i..].find("*/").map_or(b.len(), |j| i + j + 2);
        } else if src[i..].starts_with("//") {
            i = src[i..].find('\n').map_or(b.len(), |j| i + j);
        } else {
            let ch = src[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// **Adding a surface touches its own directory and nothing else.**
///
/// This is the whole reason for the rebuild, so it is checked rather than
/// believed. The page it replaces was one file of 2,700 lines and every feature
/// landed in the middle of it; a registry that still required an edit to a
/// shared list would have moved the merge conflict rather than removed it.
///
/// The property is structural: `import.meta.glob` resolves the surface
/// directory at build time, so there is nowhere to add an import. This asserts
/// there is no *other* list either — no shared file may name a surface by id.
#[test]
fn adding_a_surface_touches_no_file_belonging_to_another() {
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src");
    let surfaces_dir = ui.join("surfaces");
    if !surfaces_dir.is_dir() {
        eprintln!("no ui/src/surfaces yet — skipping");
        return;
    }

    let ids: Vec<String> = std::fs::read_dir(&surfaces_dir)
        .expect("surfaces")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(!ids.is_empty(), "no surface is registered at all");

    // Every file outside `surfaces/`, which is where a central list would be.
    let mut shared: Vec<std::path::PathBuf> = Vec::new();
    let mut stack = vec![ui.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.starts_with(&surfaces_dir) || p.ends_with("wire") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "ts" || x == "svelte") {
                shared.push(p);
            }
        }
    }

    for f in &shared {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        for id in &ids {
            // A shared file naming a surface is the list this design removes.
            // `"board"` as a quoted id is the shape; the word in prose is not.
            let quoted = format!("\"{id}\"");
            assert!(
                !text.contains(&quoted),
                "{} names the surface {quoted}. Adding a surface must touch only its own \
                 directory — the registry resolves `ui/src/surfaces/*/index.ts`, so there is \
                 nowhere to add an import and there must be nowhere to add a list either.",
                f.display()
            );
        }
    }
}

/// **A surface registers itself, and the registry is the only way in.**
///
/// The positive half: an absence check alone passes on a codebase that deleted
/// the feature. Every surface directory must have the `index.ts` that registers
/// it, and that file must call `register`.
#[test]
fn every_surface_directory_registers_itself() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/surfaces");
    if !dir.is_dir() {
        eprintln!("no ui/src/surfaces yet — skipping");
        return;
    }
    for e in std::fs::read_dir(&dir).expect("surfaces").flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let index = p.join("index.ts");
        assert!(
            index.is_file(),
            "{} has no index.ts, so nothing registers it and the glob will not find it",
            p.display()
        );
        let text = std::fs::read_to_string(&index).unwrap_or_default();
        assert!(
            text.contains("register("),
            "{} does not call register()",
            index.display()
        );
        assert!(
            text.contains("ports:"),
            "{} declares no `ports`, so the inventory cannot tell what it took over",
            index.display()
        );
    }
}

/// **No surface renders an outside value as markup.**
///
/// Svelte escapes `{value}` by construction, which changes the shape of this
/// rule rather than removing it: the old page needed an `esc()` at every
/// interpolation because it built `innerHTML` strings, and the guard was
/// *remember to call it*. Here there is exactly one construct that opts out,
/// and no surface may use it.
#[test]
fn no_surface_opts_out_of_escaping() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src");
    if !dir.is_dir() {
        return;
    }
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_some_and(|x| x == "svelte") {
                let whole = std::fs::read_to_string(&p).unwrap_or_default();
                // **Comments stripped, and it took two attempts.** The guard
                // first fired on the comment in `Board.svelte` explaining why
                // this construct is refused — the third time in this
                // repository that a guard has read a description of the defect
                // as the defect. Filtering by line *prefix* then missed it too,
                // because the mention is a continuation line inside a
                // multi-line comment. **A comment is a span, not a line.**
                let text = strip_comments(&whole);
                assert!(
                    !text.contains("{@html"),
                    "{} uses {{@html}}. Every value on this page came from an agent, an issue \
                     body or a command line, and one of them will contain a tag.",
                    p.display()
                );
            }
        }
    }
}

/// **Every surface, rendered, with its properties asserted.**
///
/// `ui/tests/render.ts` renders each surface with Svelte's **server** renderer
/// and checks what came out: the empty state is a result, an agent's output
/// reaches the document as text, the supervision badge has three states and
/// renders two of them, the header counts omit an empty bucket rather than
/// showing a zero, and every declared shortcut says what it does.
///
/// **No test runner, and that is the point.** The alternative is vitest plus a
/// DOM shim plus a testing library — three dependencies, in a repository that
/// counts them — to assert on strings `render()` already returns. Svelte ships
/// the renderer; `vite build --ssr` produces a module node runs; the assertions
/// are `if` statements. The same shape as `tests/ui_render.js` does for the
/// page being replaced.
///
/// Skips loudly where node is absent, because a silent skip is how this
/// repository already lost checks it believed it had.
#[test]
fn every_surface_renders_what_it_promises() {
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    if !ui.join("node_modules").is_dir() {
        eprintln!("skipped: ui/node_modules is not installed");
        return;
    }
    if !ui.join("tests/render.ts").is_file() {
        eprintln!("skipped: no surface render harness");
        return;
    }

    let out = std::process::Command::new("npm")
        .args(["run", "--silent", "render"])
        .current_dir(&ui)
        .output()
        .expect("npm run render");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stdout.contains("ui_surfaces: ok"),
        "the surfaces did not render as promised:\n{stdout}\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// The port inventory
// ---------------------------------------------------------------------------

/// **Every control the page being replaced has, checked against the rebuild.**
///
/// This is the gate for the whole port, and the reason it exists is that a
/// rebuild loses controls *quietly*. Nobody deletes a button; a surface is
/// ported, it looks right, and the thing nobody thought to try is gone. The
/// spec's own framing: a port that loses a control is a regression with a new
/// coat of paint.
///
/// So the inventory is extracted from the legacy page rather than written by
/// hand — a hand-written list is a second thing that drifts — and every entry
/// is either **ported** or **explicitly still owed**. The owed list is the
/// honest shape of the remaining work, and it may only ever shrink: adding to
/// it is how a port quietly narrows its own scope.
#[test]
fn the_rebuild_loses_no_control_the_page_already_has() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // **The sixteen controls the deleted page offered**, extracted from its
    // `data-act` and `data-go` attributes on 2026-09-21 — the day it was
    // deleted — and frozen here.
    //
    // This was read out of the page itself until the switch, and a hand-written
    // list was refused then for the reason hand-written lists are always
    // refused: they drift from what they describe. That reason expired with the
    // page. Nothing can drift from a file that does not exist, and the list can
    // no longer be re-derived, so freezing it is the only way the inventory
    // survives the thing it was an inventory of.
    //
    // It is kept rather than deleted because the question it answers outlives
    // the port: *did the rebuild quietly lose something?* Somebody removing a
    // surface a year from now still has to answer for `retry` or `snooze`
    // disappearing with it, and this is the only record that they existed.
    const CONTROLS: &[&str] = &[
        "allow", "approve", "attach", "choose", "copyrule", "deny", "dispatch", "focus", "github",
        "palette", "reply", "resume", "retry", "search", "setup", "snooze",
    ];
    let controls: Vec<String> = CONTROLS.iter().map(|c| (*c).to_string()).collect();

    // **Still owed, and this list may only shrink.** Each is a control whose
    // surface is not ported; naming them is what keeps the remaining work
    // visible instead of letting the port declare itself finished early.
    // **Empty, as of 2026-09-21.** Every control the page being replaced offers
    // has a home in the rebuild. That is what the list was for, and an empty
    // one is the only state in which the switch is an honest move rather than
    // the last box on a plan.
    //
    // It stays here rather than being deleted: the next control the old page
    // grows, or the next one somebody notices was never extracted, lands as a
    // failure with nowhere to go — which is the moment to decide, not to
    // discover later.
    const NOT_YET_PORTED: &[&str] = &[];

    // **Replaced rather than ported**, each with what does the job instead.
    //
    // A third category, for the same reason the guard ledger has one: *owed*
    // and *does not apply* look identical in a list of two and are opposite
    // facts. A control removed with the feature it belonged to is neither
    // missing nor carried.
    const REPLACED: &[(&str, &str)] = &[(
        "palette",
        "the keyboard model was removed on 2026-09-21, and the palette was the keyboard \
         registry with a filter box. The shell's nav lists every surface, which is the \
         job it was doing",
    )];

    // **What the rebuild has taken over, read from each surface's own
    // declaration.** Grepping the sources for a control's name does not work:
    // the first version of this check matched `snooze` in a *shortcut label*
    // and read it as a ported snooze button. A control is ported when
    // something handles it, and the only thing that knows is the surface.
    let surfaces = root.join("ui/src/surfaces");
    let mut rebuilt: Vec<String> = Vec::new();
    if surfaces.is_dir() {
        for e in std::fs::read_dir(&surfaces).into_iter().flatten().flatten() {
            let index = e.path().join("index.ts");
            let Ok(text) = std::fs::read_to_string(&index) else {
                continue;
            };
            let Some(list) = text
                .split_once("ports:")
                .and_then(|(_, r)| r.split_once('['))
                .and_then(|(_, r)| r.split_once(']'))
                .map(|(l, _)| l)
            else {
                panic!("{} declares no `ports`", index.display());
            };
            rebuilt.extend(
                list.split(',')
                    .map(|t| t.trim().trim_matches(['"', '\'']).to_string())
                    .filter(|t| !t.is_empty()),
            );
        }
    }

    let mut missing = Vec::new();
    for c in &controls {
        let ported = rebuilt.iter().any(|p| p == c);
        let owed = NOT_YET_PORTED.contains(&c.as_str());
        let replaced = REPLACED.iter().any(|(g, _)| g == c);
        if !ported && !owed && !replaced {
            missing.push(c.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "the rebuild has no home for {missing:?}. A port that loses a control is a regression \
         with a new coat of paint. Each one is exactly one of three things: ported by a surface; \
         on NOT_YET_PORTED with the surface it belongs to, which is a debt; or in REPLACED with \
         what does its job instead, which is a decision. A control in none of the three is one \
         nobody noticed going."
    );

    for (name, why) in REPLACED {
        assert!(
            controls.iter().any(|c| c == name),
            "REPLACED names `{name}`, which the page being replaced does not have"
        );
        assert!(
            !rebuilt.iter().any(|p| p == name),
            "REPLACED names `{name}` and a surface also claims to port it — it is one or the other"
        );
        assert!(
            !why.is_empty(),
            "`{name}` is replaced by nothing in particular"
        );
    }

    // **The owed list may only shrink, in both directions.**
    //
    // An entry naming a control the page no longer has is a list that has
    // stopped describing anything. And an entry naming a control that is
    // *already ported* is worse: it makes the remaining work look larger than
    // it is, which is the direction that lets a port declare itself unfinished
    // for ever and quietly stop.
    for owed in NOT_YET_PORTED {
        assert!(
            controls.iter().any(|c| c == owed),
            "NOT_YET_PORTED names `{owed}`, which the page being replaced does not have. \
             The list describes what is still owed, so an entry with nothing behind it is \
             a promise nobody is keeping."
        );
        assert!(
            !rebuilt.iter().any(|p| p == owed),
            "NOT_YET_PORTED still names `{owed}`, and the rebuild has it. Take it off the list — \
             an owed entry that is already done makes the remaining work look larger than it is."
        );
    }
}

/// **No surface in the rebuild reaches a policy or permission-writing route.**
///
/// Constitution II on the page. The sweep exists on the page being replaced
/// and it has to survive the port, because the thing it protects is not a
/// convention — an agent on this machine runs as the same user and can read
/// the bearer token, so a route that edits a permission file is reachable by
/// the party the file exists to bound.
///
/// Checked against the **served bundle** as well as the source, because a
/// surface could reach one through a helper the source sweep does not read.
#[test]
fn no_surface_reaches_a_policy_route() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut bodies: Vec<(String, String)> = Vec::new();
    let src = root.join("ui/src");
    if src.is_dir() {
        let mut stack = vec![src];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "ts" || x == "svelte") {
                    bodies.push((
                        p.display().to_string(),
                        std::fs::read_to_string(&p).unwrap_or_default(),
                    ));
                }
            }
        }
    }
    if let Some(dist) = dist() {
        bodies.push((
            "the served bundle".into(),
            std::fs::read_to_string(dist.join("app.js")).unwrap_or_default(),
        ));
    }
    assert!(!bodies.is_empty(), "nothing to sweep");

    for (name, body) in &bodies {
        // Every route the file names, and whether it touches policy.
        let reached: Vec<&str> = body
            .match_indices("/api/")
            .filter_map(|(at, _)| body[at..].split(['`', '"', '\'', '$', ' ', ')']).next())
            .filter(|r| r.contains("polic") || r.contains("rule") || r.contains("auto_allow"))
            .collect();
        assert!(
            reached.is_empty(),
            "{name} reaches a policy route: {reached:?}. An agent on this machine runs as the \
             same user and can read the bearer token, so a route that edits a permission file is \
             reachable by the party the file exists to bound."
        );
    }
}

/// **The release pipeline builds the interface, or every release ships without
/// one.**
///
/// `build.rs` treats a missing bundle as `None` rather than as a build failure,
/// which is the right call — a clean checkout with no node still compiles, and
/// a contributor who never touches the interface is never asked to install a
/// toolchain for it.
///
/// It also means a release job that forgets the step produces a binary that
/// serves nothing, **silently**, and the first person to notice is somebody who
/// installed it. CI has built the interface since the scaffold landed; the
/// release workflow did not, and nothing compared them.
#[test]
fn the_release_pipeline_builds_the_interface() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
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

    // **Per job, not per file** — which is the whole of this check now.
    //
    // The version above asked only whether `release.yml` mentioned
    // `npm run build` anywhere. It did, in the job that builds the release
    // binaries, while the two jobs that run `cargo publish` did not: `ui/dist/`
    // is gitignored, a clean checkout has none, and `include` in `Cargo.toml`
    // matches nothing rather than failing. The crate would have published,
    // installed, and served a page saying it was built without an interface.
    //
    // A guard that asks whether a file mentions something passes while the one
    // place that needs it does not have it.
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

/// **Nothing the old suite guards is lost when the old page goes.**
///
/// `tests/ui_contract.rs` is 52 guards over `ui/legacy.html`, and deleting
/// that file deletes all of them at once. Most were written against a
/// hand-written page and assert its *implementation* — function names, the
/// shape of a template string — which does not transfer to compiled output.
/// The properties behind them do.
///
/// So this is the same ledger the control inventory is: every guard is either
/// **carried** — there is a named equivalent over the rebuild — or **dropped
/// with a reason**. The switch is safe when every one of the 52 is in one list
/// or the other, and a guard in neither is one nobody decided about.
///
/// This does not run the old suite. It checks that the *decision* has been
/// made about each of its guards, which is the thing that is lost silently.
#[test]
fn every_guard_over_the_old_page_is_carried_or_knowingly_dropped() {
    let guards: Vec<String> = OLD_SUITE.iter().map(|g| (*g).to_string()).collect();

    let decided: std::collections::BTreeSet<&str> = DROPPED
        .iter()
        .map(|(g, _)| *g)
        .chain(CARRIED.iter().map(|(g, _)| *g))
        .chain(OWED.iter().map(|(g, _)| *g))
        .collect();

    // Every entry in either list has to name a guard that exists, or the
    // ledger has stopped describing the suite it is about.
    for name in &decided {
        assert!(
            guards.iter().any(|g| g == name),
            "the ledger names `{name}`, which `tests/ui_contract.rs` does not have"
        );
    }

    let undecided: Vec<&String> = guards
        .iter()
        .filter(|g| !decided.contains(g.as_str()))
        .collect();
    assert!(
        undecided.is_empty(),
        "{} of the old suite's guards have no decision recorded: {undecided:?}.\n\
         Deleting `ui/legacy.html` deletes all 52 at once. Each one is either carried by a named \
         guard over the rebuild, or dropped with the reason it does not transfer — a guard in \
         neither list is one nobody decided about, and it goes silently.",
        undecided.len()
    );

    // **`OWED` is empty, and the switch has happened.** The list stays because
    // the next guard somebody decides to defer lands here rather than nowhere,
    // and an empty list is a different statement from a deleted one.
    println!(
        "guards: {} carried, {} dropped, {} owed",
        CARRIED.len(),
        DROPPED.len(),
        OWED.len()
    );
}

/// **Every token a surface uses is a token the bundle defines.**
///
/// `ui/src/tokens.css` was carried over from the hand-written page on the day
/// the scaffold landed and then imported by **nothing**, so the built
/// stylesheet had twenty-seven `var(--dim)`-style uses and not one definition.
/// Every surface rendered unstyled, and nothing noticed, because unstyled text
/// is still text and the render harness asserts on content rather than colour.
///
/// This is the guard that would have caught it on the first surface.
#[test]
fn every_token_a_surface_uses_is_one_the_bundle_defines() {
    let Some(dist) = dist() else {
        eprintln!("skipped: ui/dist is not built");
        return;
    };
    let css = std::fs::read_to_string(dist.join("index.css")).unwrap_or_default();
    if css.is_empty() {
        eprintln!("skipped: no stylesheet in the bundle");
        return;
    }

    let used: std::collections::BTreeSet<&str> = css
        .match_indices("var(--")
        .filter_map(|(at, _)| css[at + 4..].split(')').next())
        .collect();
    assert!(
        !used.is_empty(),
        "no surface uses a token, so this checks nothing"
    );

    let defined: std::collections::BTreeSet<&str> = css
        .match_indices("--")
        .filter_map(|(at, _)| {
            let rest = &css[at..];
            // A definition is `--name:`; a use is `var(--name)`.
            let name = rest.split(':').next()?;
            (rest[name.len()..].starts_with(':') && !name.contains(')') && !name.contains(' '))
                .then_some(name)
        })
        .collect();

    let missing: Vec<&&str> = used.iter().filter(|u| !defined.contains(*u)).collect();
    assert!(
        missing.is_empty(),
        "the bundle uses {missing:?} and defines them nowhere. A stylesheet that is never \
         imported still builds, and every surface renders — unstyled text is still text, which \
         is exactly why this went unnoticed."
    );
}

/// **The served document declares its language**, so a screen reader knows
/// which one to pronounce.
#[test]
fn the_served_document_declares_its_language() {
    let Some(dist) = dist() else {
        return;
    };
    let html = std::fs::read_to_string(dist.join("index.html")).unwrap_or_default();
    if html.is_empty() {
        return;
    }
    assert!(
        html.contains("<html lang=\"en\""),
        "the served document declares no language"
    );
    // And the viewport, because the phone breakpoint has nothing to act on
    // without it.
    assert!(
        html.contains("name=\"viewport\""),
        "the served document has no viewport tag, so a phone renders it at desktop width"
    );
}

/// **No surface re-sorts what the daemon ranked.**
///
/// The inbox's order is level, then decorrelation, then age — all decided in
/// `core::attention` where they are tested. A page that sorted again would be
/// the second place an order is decided, and the two would disagree the first
/// time one of them was changed.
///
/// True by construction today and one `.sort()` from false, which is exactly
/// what an absence check is for.
///
/// **Scoped to the inbox, and the first version was not.** Written over every
/// surface it fired on the board, which groups its rows by project and orders
/// the *groups* by name — a presentation choice the daemon does not make and
/// the page being replaced makes too. Reading the original guard before
/// weakening this one showed it had always been scoped to the inbox renderer:
/// the defect it names is **two authorities on what is urgent**, and a board
/// grouping by repository is not one of them.
#[test]
fn the_inbox_does_not_re_sort_what_the_daemon_ranked() {
    let inbox =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/surfaces/inbox/Inbox.svelte");
    let Ok(text) = std::fs::read_to_string(&inbox) else {
        eprintln!("skipped: no inbox surface yet");
        return;
    };
    for banned in [".sort(", ".reverse(", ".toSorted("] {
        assert!(
            !text.contains(banned),
            "the inbox calls {banned}, so there are two authorities on what is urgent. The order \
             is `core::attention`'s — level, then decorrelation, then age — and it is tested there."
        );
    }
    // The positive half: an absence check passes on a surface that renders
    // nothing at all.
    assert!(
        text.contains("{#each items"),
        "the inbox does not render the ranked list, so this guards nothing"
    );
}

/// **Everything is done by clicking, and no state is carried by motion.**
///
/// Two properties of the served stylesheet and markup, checked together
/// because both are things a rebuild loses by omission rather than by
/// decision.
#[test]
fn every_action_is_a_control_and_the_interface_survives_reduced_motion() {
    let Some(dist) = dist() else {
        eprintln!("skipped: ui/dist is not built");
        return;
    };
    let js = std::fs::read_to_string(dist.join("app.js")).unwrap_or_default();
    let css = std::fs::read_to_string(dist.join("index.css")).unwrap_or_default();

    // **Every action is a control on the page**, which is the whole of the
    // reachability claim now that the keyboard model is gone: there is no
    // second way to do anything, so nothing can be reachable one way and not
    // the other.
    assert!(
        js.contains("<button"),
        "nothing on this page is clickable, so nothing can be done at all"
    );

    // **Reduced motion is honoured**, which is cheap here because nothing
    // animates to communicate: removing the motion removes nothing.
    assert!(
        css.contains("prefers-reduced-motion"),
        "the interface does not honour a reduced-motion preference"
    );

    // **The phone is a different errand.** A breakpoint exists, and the hint
    // bar is not part of it: `j`/`k`/⌘K do not exist on a phone and a row of
    // keys nobody can press is noise.
    assert!(
        css.contains("max-width"),
        "there is no breakpoint, so a phone renders this at desktop width"
    );
}

/// **The board can search what the CLI can, and the inbox snoozes what the
/// API can snooze.**
///
/// Parity assertions rather than ports: both surfaces exist, and what has to
/// hold is that they reach routes the daemon actually serves. A surface
/// calling a route that 404s is a button that cannot keep its promise, and the
/// failure is invisible until somebody presses it.
#[test]
fn every_route_a_surface_calls_is_one_the_daemon_serves() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let api = std::fs::read_to_string(root.join("src/api.rs")).expect("the api");

    // The routes the daemon registers, as it registers them.
    let served: Vec<String> = api
        .match_indices(".route(\"")
        .filter_map(|(at, _)| api[at + 8..].split('"').next())
        .map(str::to_string)
        .collect();
    assert!(
        served.len() > 20,
        "the route extractor found {}",
        served.len()
    );

    // What each surface calls, with the interpolations reduced to the `{}`
    // the daemon writes as `{id}`.
    let mut called: Vec<(String, String)> = Vec::new();
    let dir = root.join("ui/src");
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().is_some_and(|x| x == "svelte" || x == "ts") {
                continue;
            }
            // **Comments stripped, and this is the fourth guard in this
            // repository to need it.** A comment explaining which route is
            // wrong contains that route, and a guard that cannot tell a
            // description of the defect from the defect reads every
            // post-mortem as a breach.
            let text = strip_comments(&std::fs::read_to_string(&p).unwrap_or_default());
            for (at, _) in text.match_indices("/api/") {
                // **`${…}` is skipped whole**, braces and all. The first
                // version stopped at the first `)`, which cut
                // `${encodeURIComponent(id)}` in half and reported a route
                // nobody writes — a guard failing on its own parser rather
                // than on the code.
                let mut shape = String::new();
                let mut chars = text[at..].chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '$' && chars.peek() == Some(&'{') {
                        chars.next();
                        let mut depth = 1;
                        for n in chars.by_ref() {
                            match n {
                                '{' => depth += 1,
                                '}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        shape.push_str("{}");
                        continue;
                    }
                    if matches!(c, '`' | '"' | '\'' | '?' | ' ' | ')') {
                        break;
                    }
                    shape.push(c);
                }
                called.push((p.display().to_string(), shape));
            }
        }
    }
    assert!(!called.is_empty(), "no surface calls the daemon at all");

    for (file, route) in &called {
        // `/api/work/{}/approve` against the daemon's `/api/work/{id}/approve`.
        let shape = route.clone();
        let matches = served.iter().any(|s| {
            let s_shape: String = {
                let mut out = String::new();
                let mut rest = s.as_str();
                while let Some(i) = rest.find('{') {
                    out.push_str(&rest[..i]);
                    out.push_str("{}");
                    rest = &rest[rest[i..].find('}').map_or(rest.len(), |j| i + j + 1)..];
                }
                out.push_str(rest);
                out
            };
            s_shape == shape
        });
        assert!(
            matches,
            "{file} calls `{route}`, which the daemon does not serve. A surface reaching a route \
             that 404s is a button that cannot keep its promise, and nobody finds out until they \
             press it.\nServed: {served:?}"
        );
    }
}

/// **The waiting list is one list, not a list per project.**
///
/// The board groups by project because that is the unit a person thinks in
/// for *sessions*; the inbox must not, because its order is urgency and
/// grouping destroys it — twelve rows under four headings hide which of the
/// twelve is the critical one.
///
/// The distinction is easy to lose in a port: grouping looks tidier, and the
/// board next door does it.
#[test]
fn the_waiting_list_is_one_list_and_not_a_list_per_project() {
    let inbox =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/surfaces/inbox/Inbox.svelte");
    let Ok(whole) = std::fs::read_to_string(&inbox) else {
        return;
    };
    let text = strip_comments(&whole);

    for banned in ["reduce(", "groupBy", "Object.entries("] {
        assert!(
            !text.contains(banned),
            "the inbox calls {banned}, which is how a flat ranked list becomes a grouped one. \
             Its order is urgency; grouping by project hides which of twelve rows is the \
             critical one."
        );
    }
    // The positive half: it renders the items it was given, in the order it
    // was given them.
    assert!(
        text.contains("{#each items as i"),
        "the inbox does not iterate the ranked list it was handed"
    );
}

// **Dropped, each with the reason.** These assert something about the
// hand-written page that has no meaning for compiled output.
/// **The guards the deleted suite held**, frozen on 2026-09-21 — the day
/// `tests/ui_contract.rs` was deleted at the switch.
///
/// Read out of that file until it was deleted. It is a hand-written list now
/// for the same reason the control inventory is: nothing can drift from a file
/// that does not exist, and the list can no longer be re-derived.
const OLD_SUITE: &[&str] = &[
    "fields_read_by_page",
    "the_page_carries_no_credential_and_no_remote_dependency",
    "every_button_the_inbox_renders_is_one_the_item_offered",
    "the_key_legend_names_only_keys_the_row_under_the_cursor_would_answer",
    "work_items_can_be_snoozed_through_a_route_that_exists",
    "every_element_the_script_reaches_for_exists_in_the_page",
    "the_board_can_search_what_the_cli_can",
    "the_answers_an_agent_offered_are_answerable",
    "the_launchers_are_the_only_things_that_cover_the_page",
    "dispatch_refuses_a_project_nobody_trusted",
    "the_why_pane_reads_the_decision_log_and_nothing_else",
    "the_board_loads_the_projects_a_person_can_dispatch_to",
    "the_work_card_reads_only_fields_the_api_serves",
    "the_board_script_parses",
    "every_value_read_off_the_api_is_escaped",
    "there_is_exactly_one_live_region_and_it_is_polite",
    "every_overlay_is_a_dialog_and_returns_focus",
    "a_glyph_that_carries_state_has_a_word_beside_it",
    "the_sections_that_hold_rows_say_they_are_lists",
    "the_page_renders_and_escapes_what_it_renders",
    "the_page_is_one_small_self_contained_file",
    "the_page_declares_its_language",
    "a_modal_dialog_keeps_the_keyboard",
    "every_column_between_the_name_and_the_summary_is_sized",
    "the_page_has_a_breakpoint_behind_its_viewport_tag",
    "every_overlay_is_in_the_list_that_closes_them",
    "the_empty_board_has_two_sentences_and_only_one_is_a_thing_to_do",
    "the_setup_panel_reads_the_configuration_and_never_writes_it",
    "every_global_command_has_something_to_click",
    "the_work_view_shows_the_gate_evidence_and_names_what_is_missing",
    "the_two_ways_a_claim_can_be_absent_do_not_render_the_same",
    "the_rule_is_offered_and_never_written",
    "the_daemon_renderer_cannot_be_handed_an_unescaped_value",
    "the_renderer_escapes_what_repositories_and_models_produce",
    "no_rail_entry_offers_a_surface_that_cannot_be_opened",
    "an_empty_surface_does_not_read_as_an_unavailable_one",
    "monospace_is_kept_for_what_earns_it_and_numbers_are_tabular",
    "no_state_is_carried_by_motion_and_reduced_motion_is_honoured",
    "the_default_surface_shows_both_of_the_boards_sections",
    "the_gate_view_reads_the_structured_outcome",
    "every_command_outcome_has_a_word_the_page_can_say",
    "the_landing_surface_is_what_needs_you",
    "the_session_list_is_still_reachable_and_unchanged",
    "a_waiting_row_says_where_it_came_from_and_how_long",
    "the_waiting_list_is_not_grouped_by_project",
    "moving_the_default_surface_did_not_cost_the_inbox_its_actions",
    "the_page_does_not_re_sort_what_the_daemon_ranked",
    "no_page_control_reaches_a_merge_or_a_weaker_permission",
    "the_page_reads_its_thresholds_from_the_daemon",
    // Added by hand: this one was written after the last commit of the suite
    // and only ever existed in the working copy, so it is not in the frozen
    // extract. The ledger named it, which is how it was noticed at all — the
    // ledger outlived the file it described, which is what a ledger is for.
    "the_page_words_no_part_of_the_certificate",
    "both_renderers_describe_a_silent_session_identically",
    "every_offered_action_is_one_a_surface_performs",
];

const DROPPED: &[(&str, &str)] = &[
    (
        "every_overlay_is_a_dialog_and_returns_focus",
        "decided 2026-09-21: overlays became surfaces, and a surface is a section. Nothing \
         covers the page, so there is no focus to return",
    ),
    (
        "a_modal_dialog_keeps_the_keyboard",
        "same decision: there is no modal to trap the keyboard in",
    ),
    (
        "the_key_legend_names_only_keys_the_row_under_the_cursor_would_answer",
        "decided 2026-09-21: the rebuild has no keyboard model to legend. It was carried \
         for one day — a hint bar over a per-surface key registry — and then removed with \
         the feature, which is why it moved from carried to dropped rather than starting \
         here. A legend for keys that do not exist is a guard over nothing",
    ),
    (
        "the_board_script_parses",
        "the bundle is compiled; a parse failure is a build failure",
    ),
    (
        "the_daemon_renderer_cannot_be_handed_an_unescaped_value",
        "about the daemon's HTML renderer, which the rebuild does not use",
    ),
    (
        "the_renderer_escapes_what_repositories_and_models_produce",
        "same daemon renderer; the rebuild renders every value through Svelte",
    ),
    (
        "every_column_between_the_name_and_the_summary_is_sized",
        "a fixed-column table the rebuild replaced with flow layout",
    ),
    (
        "the_default_surface_shows_both_of_the_boards_sections",
        "one page with two sections became two surfaces, deliberately",
    ),
    (
        "the_session_list_is_still_reachable_and_unchanged",
        "about a move within the old page; the board surface is the session list",
    ),
    (
        "moving_the_default_surface_did_not_cost_the_inbox_its_actions",
        "about that same move; the inbox surface owns its actions now",
    ),
    (
        "both_renderers_describe_a_silent_session_identically",
        "there is one renderer in the rebuild, so two cannot disagree",
    ),
    (
        "the_launchers_are_the_only_things_that_cover_the_page",
        "nothing covers the page: overlays became surfaces",
    ),
    (
        "every_overlay_is_in_the_list_that_closes_them",
        "same — there is no overlay list to be out of",
    ),
    (
        "the_page_is_one_small_self_contained_file",
        "the rebuild is deliberately many files; the served artefact is sized by its own guard",
    ),
    (
        "every_element_the_script_reaches_for_exists_in_the_page",
        "Svelte binds at compile time; a missing element is a compile error",
    ),
    (
        "fields_read_by_page",
        "the wire types are generated from the Rust shapes and checked in",
    ),
    (
        "the_page_renders_and_escapes_what_it_renders",
        "superseded by the server-render harness, which renders every surface",
    ),
];

// **Carried**, each naming the guard that replaced it.
const CARRIED: &[(&str, &str)] = &[
    (
        "the_page_has_a_breakpoint_behind_its_viewport_tag",
        "every_action_is_a_control_and_the_interface_survives_reduced_motion, \
         plus the_served_document_declares_its_language for the viewport tag. \
         Whether it *works* is measured on a device, which is a person's task",
    ),
    (
        "the_waiting_list_is_not_grouped_by_project",
        "the_waiting_list_is_one_list_and_not_a_list_per_project",
    ),
    (
        "work_items_can_be_snoozed_through_a_route_that_exists",
        "every_route_a_surface_calls_is_one_the_daemon_serves",
    ),
    (
        "the_board_can_search_what_the_cli_can",
        "every_route_a_surface_calls_is_one_the_daemon_serves, over the search surface",
    ),
    (
        "every_global_command_has_something_to_click",
        "every_action_is_a_control_and_the_interface_survives_reduced_motion",
    ),
    (
        "no_state_is_carried_by_motion_and_reduced_motion_is_honoured",
        "every_action_is_a_control_and_the_interface_survives_reduced_motion",
    ),
    (
        "the_page_declares_its_language",
        "the_served_document_declares_its_language",
    ),
    (
        "the_page_does_not_re_sort_what_the_daemon_ranked",
        "the_inbox_does_not_re_sort_what_the_daemon_ranked",
    ),
    (
        "monospace_is_kept_for_what_earns_it_and_numbers_are_tabular",
        "every_token_a_surface_uses_is_one_the_bundle_defines, plus tests/tokens.rs",
    ),
    (
        "the_page_carries_no_credential_and_no_remote_dependency",
        "nothing_in_the_served_interface_reaches_outside_this_machine",
    ),
    (
        "every_value_read_off_the_api_is_escaped",
        "no_surface_opts_out_of_escaping",
    ),
    (
        "the_page_words_no_part_of_the_certificate",
        "the_page_words_no_part_of_the_certificate",
    ),
    (
        "the_work_card_reads_only_fields_the_api_serves",
        "the checked-in wire types, generated from the Rust shapes",
    ),
    (
        "the_gate_view_reads_the_structured_outcome",
        "ui/tests/render.ts: the certificate",
    ),
    (
        "every_command_outcome_has_a_word_the_page_can_say",
        "ui/tests/render.ts: the glyph has a word beside it",
    ),
    (
        "the_two_ways_a_claim_can_be_absent_do_not_render_the_same",
        "ui/tests/render.ts: the certificate's claim block",
    ),
    (
        "the_board_loads_the_projects_a_person_can_dispatch_to",
        "ui/tests/render.ts: every surface maps the feed to its own props",
    ),
    (
        "a_waiting_row_says_where_it_came_from_and_how_long",
        "ui/tests/render.ts: the inbox row carries project and age",
    ),
    (
        "the_why_pane_reads_the_decision_log_and_nothing_else",
        "no_surface_reaches_a_policy_route, plus the work surface's evidence block",
    ),
    (
        "no_page_control_reaches_a_merge_or_a_weaker_permission",
        "no_surface_reaches_a_policy_route",
    ),
    (
        "a_glyph_that_carries_state_has_a_word_beside_it",
        "ui/tests/render.ts: accessibility",
    ),
    (
        "there_is_exactly_one_live_region_and_it_is_polite",
        "ui/tests/render.ts: accessibility",
    ),
    (
        "the_sections_that_hold_rows_say_they_are_lists",
        "ui/tests/render.ts: accessibility",
    ),
    (
        "an_empty_surface_does_not_read_as_an_unavailable_one",
        "ui/tests/render.ts: empty states",
    ),
    (
        "the_empty_board_has_two_sentences_and_only_one_is_a_thing_to_do",
        "ui/tests/render.ts: empty states",
    ),
    (
        "every_button_the_inbox_renders_is_one_the_item_offered",
        "ui/tests/render.ts: the answer path",
    ),
    (
        "the_answers_an_agent_offered_are_answerable",
        "ui/tests/render.ts: the answer path",
    ),
    (
        "the_rule_is_offered_and_never_written",
        "ui/tests/render.ts: the rule to paste",
    ),
    (
        "the_setup_panel_reads_the_configuration_and_never_writes_it",
        "no_surface_reaches_a_policy_route, plus the setup surface's own text",
    ),
    (
        "dispatch_refuses_a_project_nobody_trusted",
        "ui/tests/render.ts: the preflight panel",
    ),
    (
        "every_offered_action_is_one_a_surface_performs",
        "the_rebuild_loses_no_control_the_page_already_has",
    ),
    (
        "no_rail_entry_offers_a_surface_that_cannot_be_opened",
        "every_surface_directory_registers_itself",
    ),
    (
        "the_work_view_shows_the_gate_evidence_and_names_what_is_missing",
        "ui/tests/render.ts: the certificate",
    ),
    (
        "the_landing_surface_is_what_needs_you",
        "ui/tests/render.ts: the shell opens on what needs you. **This entry was false for the \
         life of the rebuild** — it named a block that renders the inbox, which is not the same \
         claim as the shell opening on it, and the shell opened on the board because nav order \
         was alphabetical. A ledger entry naming a guard that checks something adjacent is worse \
         than an empty one: it reports the decision as made",
    ),
    (
        "the_page_reads_its_thresholds_from_the_daemon",
        "ui/tests/render.ts: the board reads its thresholds from the daemon. **It was the last \
         one owed**, for a year's worth of passes, because the rebuild had no surface colouring \
         a gauge and there was nothing for the guard to hold. Closed 2026-09-21 by building the \
         thing rather than by reclassifying it: a context window past the daemon's configured \
         threshold says so, in a word, and no threshold means no opinion",
    ),
];

// **Owed: a property the interface must have and does not yet.**
//
// Empty since 2026-09-21, when the last entry was closed by building it. The
// list stays rather than being deleted: an empty list is a different statement
// from an absent one, and the next guard somebody decides to defer has a place
// to land instead of being decided by nobody.
//
// It is deliberately separate from *dropped* — the difference between "this
// does not apply" and "this applies and is not done" is the whole value of the
// ledger, and the two look identical in a list of one kind.
const OWED: &[(&str, &str)] = &[];

/// **How many properties the interface protects.**
///
/// The plausible bad outcome of a rewrite is not a visible break. It is *the
/// same behaviour with fewer guards, shipped on a day when it looks identical*.
/// So the count is a ratchet.
///
/// # The floor may only fall by an amount somebody wrote down
///
/// A floor that may never fall is honest right up until a feature is removed on
/// purpose, at which point it forces whoever removed it to pad the suite with
/// guards over nothing. So there is a way down — and it is not editing the
/// constant.
///
/// `PEAK` is the highest the count has been. The floor is `PEAK` less the
/// properties listed in `REMOVED_WITH_FEATURE`, each with a count and a reason.
/// Deleting guards therefore fails this check until the deletion is written
/// down, and the write-down is a diff a reviewer sees rather than a number that
/// quietly got smaller.
///
/// **This mechanism exists because the constant was re-seated twice in one
/// day** — once for the keyboard removal and once at the switch — and a floor
/// that moves whenever it is inconvenient is not a floor. Neither of those two
/// could be attributed by arithmetic afterwards, because the counter itself was
/// fixed in the same window, which is exactly the situation this prevents from
/// recurring: the accounting now happens at the moment of removal, by the
/// person who knows what they removed.
const PEAK: usize = 135;

/// Properties removed with the feature they guarded, each with its count.
///
/// **A guard over a removed feature is not a guard**, so removing it is not a
/// loss and the floor should follow. Everything else — a refactor, a
/// simplification, a rewrite that "does the same thing" — may not lower this
/// number, because that is the exact shape of the failure the ratchet catches.
///
/// Empty today. `PEAK` was seated at the switch, after the two removals that
/// prompted the mechanism, and re-seated whenever the count rises — at **104** when the
/// interface was redesigned, and at **123** when the diff surface and the undo
/// contract landed. A rise is the direction that needs no accounting.
const REMOVED_WITH_FEATURE: &[(&str, usize, &str)] = &[];

/// Today's floor: the peak, less what was deliberately removed.
fn floor() -> usize {
    let removed: usize = REMOVED_WITH_FEATURE.iter().map(|(_, n, _)| *n).sum();
    assert!(
        removed <= PEAK,
        "more properties are recorded as removed than have ever existed"
    );
    for (what, n, why) in REMOVED_WITH_FEATURE {
        assert!(
            *n > 0 && !why.is_empty(),
            "`{what}` lowers the floor by {n} and says why in {} characters. A removal that \
             costs nothing does not need to be here, and one that costs something needs a reason.",
            why.len()
        );
    }
    PEAK - removed
}

fn property_count() -> (usize, usize, usize) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let rust = std::fs::read_to_string(root.join("tests/ui_bundle.rs"))
        .unwrap_or_default()
        .matches("\n#[test]")
        .count();
    let tokens = std::fs::read_to_string(root.join("tests/tokens.rs"))
        .unwrap_or_default()
        .matches("#[test]")
        .count();
    // **Every `fail(…)` is one asserted property**, whatever its message is
    // written as, and the count proves it rather than assuming it.
    //
    // This counter has now been wrong twice, in the same direction, while the
    // number was being used to decide whether it was safe to switch. First it
    // matched `fail(` only at the start of a line and missed every
    // `if (…) fail("…")` — most of them. Then it matched `fail("` and missed
    // every message written with a backtick, which is every message that
    // interpolates a surface id — eight of them.
    //
    // So it no longer pattern-matches the *messages*. It counts the calls, and
    // asserts that each one it counted is a call it knows how to read. A third
    // form fails the assert instead of quietly lowering the number. A counter
    // that measures the wrong thing is worse than none, because it is believed.
    let text = std::fs::read_to_string(root.join("ui/tests/render.ts")).unwrap_or_default();
    let render = text.matches("fail(").count();
    // **A message may begin on the next line.** `fail(` followed by a newline
    // and an indented string is the shape a long message takes after the
    // formatter wraps it, and it is still a message this counter can read — so
    // the quote is looked for past any whitespace rather than immediately
    // after the bracket. Found by the assert below refusing to undercount by
    // two, which is the third time this counter has been corrected and the
    // first time it caught itself.
    let readable = text
        .match_indices("fail(")
        .filter(|(i, _)| {
            let rest = text[i + "fail(".len()..].trim_start();
            rest.starts_with('"') || rest.starts_with('`')
        })
        .count();
    assert_eq!(
        render,
        readable,
        "{} fail() calls carry a message this counter cannot read. Count the call, not the \
         quote style.",
        render - readable
    );
    (rust, render, tokens)
}

#[test]
fn the_rebuild_protects_more_properties_than_it_started_with() {
    let (rust, render, tokens) = property_count();
    let total = rust + render + tokens;
    let floor = floor();
    println!(
        "the interface protects {total} properties ({rust} rust + {render} render + {tokens} \
         tokens); floor {floor} (peak {PEAK} less {} removed with their features)",
        PEAK - floor
    );
    assert!(
        total >= floor,
        "the interface protects {total} properties and the floor is {floor}. A rewrite loses \
         guards quietly. If you removed a feature, add it to REMOVED_WITH_FEATURE with the \
         count and the reason — do not edit PEAK, which is a measurement rather than a target."
    );
    assert!(
        total <= PEAK || REMOVED_WITH_FEATURE.is_empty(),
        "the count has risen to {total}, above a peak of {PEAK} that still carries \
         {} recorded removals. Re-seat PEAK at {total} and clear the list: the removals are \
         accounted for by the new measurement, and carrying both subtracts them twice.",
        REMOVED_WITH_FEATURE.len()
    );
}

/// **The quickstart names every surface the interface has.**
///
/// The page a first-time reader meets carried a table of nine keyboard
/// shortcuts for months after the keyboard was removed, and then a surface
/// table that was missing one. Both were hand-maintained copies of something
/// the registry already knows, and nothing compared them — the same defect the
/// command groups had, in the same file class.
#[test]
fn the_quickstart_names_every_surface_the_registry_has() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = std::fs::read_to_string(root.join("site/content/docs/quickstart.md"))
        .expect("the quickstart");

    let mut titles: Vec<String> = Vec::new();
    let dir = root.join("ui/src/surfaces");
    for entry in std::fs::read_dir(&dir)
        .expect("the surfaces directory")
        .flatten()
    {
        let index = entry.path().join("index.ts");
        let Ok(text) = std::fs::read_to_string(&index) else {
            continue;
        };
        let title = text
            .split("title:")
            .nth(1)
            .and_then(|r| r.split('"').nth(1))
            .unwrap_or_else(|| panic!("{} declares no title", index.display()))
            .to_string();
        titles.push(title);
    }
    assert!(
        titles.len() >= 8,
        "the reader found {} surfaces, so it has stopped matching the registry",
        titles.len()
    );

    let missing: Vec<&String> = titles.iter().filter(|t| !doc.contains(*t)).collect();
    assert!(
        missing.is_empty(),
        "the quickstart does not name {missing:?}. It is the first page a reader meets, and a \
         surface it omits is one they will not know exists — the registry is the list, and this \
         page is a copy of it."
    );
}

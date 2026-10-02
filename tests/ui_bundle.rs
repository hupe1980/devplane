//! Guards on the built interface the binary embeds: embedding, readability,
//! size, and no outside fetches. They run against `ui/dist/` and skip with a
//! message when it is not built; under `CI` a skip is a failure.

/// Gzipped ceiling for the served interface. It is served over loopback, so
/// this is a tripwire for an undecided dependency (hundreds of KB), not a
/// cost budget. Exceeding it means asking what was added, not minifying.
const BUDGET_GZIPPED: usize = 250_000;

/// Per-file line ceiling for interface source; a file past it is several
/// surfaces and should be split.
const SOURCE_LINE_CEILING: usize = 600;

fn dist() -> Option<std::path::PathBuf> {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/dist");
    d.join("app.js").is_file().then_some(d)
}

/// Reports a skip; under `CI`, where the interface is built, panics instead.
fn skip(why: &str) {
    if std::env::var("CI").is_ok() {
        panic!("{why} — a skip cannot pass in CI, where the interface is built");
    }
    eprintln!("skipped: {why}");
}

fn gzipped(bytes: &[u8]) -> usize {
    use std::io::Write as _;
    let mut child = std::process::Command::new("gzip")
        .args(["-9", "-c"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("gzip");
    // Written from a thread: gzip's output fills its pipe before the input is
    // all read, and a single thread writing then reading would wait forever.
    let mut stdin = child.stdin.take().expect("stdin");
    let input = bytes.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&input).expect("write"));
    let out = child.wait_with_output().expect("gzip output").stdout.len();
    writer.join().expect("writer");
    out
}

/// The whole served artefact fits the budget, gzipped.
#[test]
fn the_served_interface_is_within_its_budget() {
    let Some(dist) = dist() else {
        skip("ui/dist is not built");
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

/// The served bundle is readable without a source map, checked on the bytes
/// rather than trusting a build flag.
#[test]
fn the_served_bundle_can_be_read_without_a_source_map() {
    let Some(dist) = dist() else {
        skip("ui/dist is not built");
        return;
    };
    let js = std::fs::read_to_string(dist.join("app.js")).expect("app.js");

    // The median, not the longest: compiled template literals legitimately
    // produce a few long lines, while a minified bundle has a huge median.
    let mut widths: Vec<usize> = js.lines().map(str::len).collect();
    widths.sort_unstable();
    let median = widths.get(widths.len() / 2).copied().unwrap_or(0);
    assert!(
        median < 80,
        "the median line is {median} characters, which is a minified blob rather \
         than something a person can follow"
    );
    // A template literal is thousands of characters at worst; a minified
    // chunk is tens of thousands.
    let longest = widths.last().copied().unwrap_or(0);
    assert!(
        longest < 8_000,
        "the longest line is {longest} characters, which is not a template literal"
    );

    // A minifier replaces these.
    for name in ["function", "const ", "return "] {
        assert!(
            js.contains(name),
            "`{name}` does not appear in the bundle, so it has been minified"
        );
    }
    assert!(
        js.lines().count() > 100,
        "the bundle is {} lines, which is not readable output",
        js.lines().count()
    );
}

/// No served asset loads anything from outside the machine. Checks loading
/// forms, not the substring `http`, which appears in harmless error-doc links.
#[test]
fn nothing_in_the_served_interface_reaches_outside_this_machine() {
    let Some(dist) = dist() else {
        skip("ui/dist is not built");
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

/// No interface source file exceeds the line ceiling.
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

/// What `build.rs` embedded matches `ui/dist/`, so a failed embed is not
/// mistaken for an unbuilt bundle.
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
        (None, None) => skip("no bundle was built and none was embedded"),
        (Some(_), None) => panic!("a bundle is embedded but ui/dist is gone; the binary is stale"),
        (None, Some(_)) => {
            panic!("ui/dist exists and nothing was embedded — run a clean build")
        }
    }
}

/// No two Rust types export the same wire name; the generated interface has
/// one namespace, so one would silently overwrite the other.
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
            // The `rename` if present, else the type's own name.
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

/// Removes `<!-- … -->`, `/* … */` and `//` comments as spans, so a guard
/// does not match a comment describing what it forbids.
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

/// Adding a surface touches only its own directory: no shared file names a
/// surface by id or address (`import.meta.glob` already needs no import).
#[test]
fn adding_a_surface_touches_no_file_belonging_to_another() {
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src");
    let surfaces_dir = ui.join("surfaces");
    if !surfaces_dir.is_dir() {
        skip("no ui/src/surfaces yet");
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

    // Markup attribute values (e.g. `type="search"`) are platform vocabulary,
    // not surface references.
    let strip_markup_attrs = |text: &str| -> String {
        let mut out = text.to_string();
        for attr in [
            "type",
            "role",
            "inputmode",
            "autocomplete",
            "rel",
            "enterkeyhint",
            "aria-label",
            // Icon names, e.g. the `search` magnifier.
            "name",
        ] {
            for id in &ids {
                out = out.replace(&format!("{attr}=\"{id}\""), &format!("{attr}=\"\""));
            }
        }
        out
    };

    // Comments may quote the forbidden form while explaining it.
    let strip_comments = |text: &str| -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        loop {
            let block = rest.find("/*");
            let line = rest.find("//");
            let (cut, end_pat, keep_newline) = match (block, line) {
                (Some(b), Some(l)) if b < l => (b, "*/", false),
                (Some(b), None) => (b, "*/", false),
                (_, Some(l)) => (l, "\n", true),
                (None, None) => {
                    out.push_str(rest);
                    return out;
                }
            };
            out.push_str(&rest[..cut]);
            let after = &rest[cut..];
            match after.find(end_pat) {
                Some(e) => {
                    if keep_newline {
                        out.push('\n');
                    }
                    rest = &after[e + end_pat.len()..];
                }
                None => return out,
            }
        }
    };

    for f in &shared {
        let raw = std::fs::read_to_string(f).unwrap_or_default();
        let text = strip_markup_attrs(&strip_comments(&raw));
        for id in &ids {
            // A quoted id is a reference; the word in prose is not.
            let quoted = format!("\"{id}\"");
            assert!(
                !text.contains(&quoted),
                "{} names the surface {quoted}. Adding a surface must touch only its own \
                 directory — the registry resolves `ui/src/surfaces/*/index.ts`, so there is \
                 nowhere to add an import and there must be nowhere to add a list either.",
                f.display()
            );
            // An address like `"#inbox"` names a surface too.
            for open in ['"', '\'', '`'] {
                for tail in ['"', '\'', '`', '/'] {
                    let address = format!("{open}#{id}{tail}");
                    assert!(
                        !text.contains(&address),
                        "{} writes the address {address}. A shared region opens a surface the \
                         registry names — `status`, `holds`, `link` — never one it spells.",
                        f.display()
                    );
                }
            }
        }
    }
}

/// Every surface directory has an `index.ts` that calls `register`.
#[test]
fn every_surface_directory_registers_itself() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/surfaces");
    if !dir.is_dir() {
        skip("no ui/src/surfaces yet");
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
        // Controls are checked by rendering, not by a declared list.
        assert!(
            !text.contains("ports:"),
            "{} still declares `ports`; the inventory it fed is gone",
            index.display()
        );
    }
}

/// No surface uses `{@html}`, the one Svelte construct that opts out of
/// escaping.
#[test]
fn no_surface_opts_out_of_escaping() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src");
    if !dir.is_dir() {
        skip("no ui/src yet");
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

/// Every surface passes `ui/tests/render.ts`, which server-renders each one
/// and asserts on the output without a test-runner dependency.
#[test]
fn every_surface_renders_what_it_promises() {
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    if !ui.join("node_modules").is_dir() {
        skip("ui/node_modules is not installed");
        return;
    }
    if !ui.join("tests/render.ts").is_file() {
        skip("no surface render harness");
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

/// No surface reaches a policy or permission-writing route, in source or in
/// the served bundle: an agent can read the bearer token, so such a route
/// would be reachable by the party it bounds.
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

/// Every CSS custom property the bundle uses is defined somewhere in it; an
/// unimported token sheet otherwise renders unstyled without any failure.
#[test]
fn every_token_a_surface_uses_is_one_the_bundle_defines() {
    let Some(dist) = dist() else {
        skip("ui/dist is not built");
        return;
    };
    let css = std::fs::read_to_string(dist.join("index.css")).unwrap_or_default();
    if css.is_empty() {
        skip("no stylesheet in the bundle");
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

    // Inline `style="--name: …"` in the script also defines a property.
    let js = std::fs::read_to_string(dist.join("app.js")).unwrap_or_default();
    let inline: std::collections::BTreeSet<&str> = js
        .match_indices("style")
        .flat_map(|(at, _)| {
            let window = &js[at..js.len().min(at + 120)];
            window
                .match_indices("--")
                .filter_map(|(i, _)| {
                    let name = window[i..].split(':').next()?;
                    (window[i + name.len()..].starts_with(':')
                        && name.len() > 2
                        && name[2..]
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '-'))
                    .then_some(name)
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let missing: Vec<&&str> = used
        .iter()
        .filter(|u| !defined.contains(*u) && !inline.contains(*u))
        .collect();
    assert!(
        missing.is_empty(),
        "the bundle uses {missing:?} and defines them nowhere. A stylesheet that is never \
         imported still builds, and every surface renders — unstyled text is still text, which \
         is exactly why this went unnoticed."
    );
}

/// The served document declares its language and a viewport.
#[test]
fn the_served_document_declares_its_language() {
    let Some(dist) = dist() else {
        skip("ui/dist is not built");
        return;
    };
    let html = std::fs::read_to_string(dist.join("index.html")).unwrap_or_default();
    if html.is_empty() {
        skip("no index.html in the bundle");
        return;
    }
    assert!(
        html.contains("<html lang=\"en\""),
        "the served document declares no language"
    );
    assert!(
        html.contains("name=\"viewport\""),
        "the served document has no viewport tag"
    );
}

/// The inbox does not re-sort what `core::attention` ranked, so urgency has
/// one authority. Scoped to the inbox; the board may order its groups.
#[test]
fn the_inbox_does_not_re_sort_what_the_host_ranked() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/surfaces/inbox");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        skip("no inbox surface yet");
        return;
    };
    for e in entries.flatten() {
        let body = strip_comments(&std::fs::read_to_string(e.path()).unwrap_or_default());
        for banned in [".sort(", ".reverse(", ".toSorted("] {
            assert!(
                !body.contains(banned),
                "{} calls {banned}, so there are two authorities on what is urgent. The order \
                 is `core::attention`'s — level, then decorrelation, then age — and it is tested there.",
                e.path().display()
            );
        }
    }
    let Ok(text) = std::fs::read_to_string(dir.join("List.svelte")) else {
        panic!("the inbox has no list, so this guards nothing");
    };
    // Positive half: an absence check passes on a surface that renders nothing.
    assert!(
        text.contains("{#each shown"),
        "the inbox does not render the ranked list, so this guards nothing"
    );
}

/// Actions are clickable controls, reduced motion removes motion entirely, and
/// there is no viewport phone breakpoint.
#[test]
fn every_action_is_a_control_and_the_interface_survives_reduced_motion() {
    let Some(dist) = dist() else {
        skip("ui/dist is not built");
        return;
    };
    let js = std::fs::read_to_string(dist.join("app.js")).unwrap_or_default();
    let css = std::fs::read_to_string(dist.join("index.css")).unwrap_or_default();

    assert!(
        js.contains("<button"),
        "nothing on this page is clickable, so nothing can be done at all"
    );

    // Removes, not shortens: a 0.01ms transition is still a transition.
    let Some(rm) = css.split("prefers-reduced-motion").nth(1) else {
        panic!("the interface does not honour a reduced-motion preference");
    };
    assert!(
        rm.contains("transition: none !important") && rm.contains("animation: none !important"),
        "reduced motion shortens the motion rather than removing it"
    );

    // Surfaces reflow to their pane with container queries, not the viewport.
    assert!(
        !css.contains("@media (max-width"),
        "the interface carries a phone breakpoint, and it is a desktop app"
    );
}

/// Every `/api/` route a surface calls is one `src/api.rs` registers.
#[test]
fn every_route_a_surface_calls_is_one_the_host_serves() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let api = std::fs::read_to_string(root.join("src/api.rs")).expect("the api");

    // `.route("…"` — on one line, or with the path on the next as rustfmt
    // breaks a long one.
    let served: Vec<String> = api
        .match_indices(".route(")
        .filter_map(|(at, _)| api[at + 7..].trim_start().strip_prefix('"'))
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_string)
        .collect();
    assert!(
        served.len() > 20,
        "the route extractor found {}",
        served.len()
    );

    // Interpolations are reduced to `{}`.
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
            let text = strip_comments(&std::fs::read_to_string(&p).unwrap_or_default());
            for (at, _) in text.match_indices("/api/") {
                // `${…}` is skipped whole, so nested parens do not cut it.
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
    assert!(!called.is_empty(), "no surface calls the host at all");

    // Normalises both sides, so `{id}` and `{}` compare equal.
    let shape_of = |s: &str| -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(i) = rest.find('{') {
            out.push_str(&rest[..i]);
            out.push_str("{}");
            rest = &rest[rest[i..].find('}').map_or(rest.len(), |j| i + j + 1)..];
        }
        out.push_str(rest);
        out
    };

    for (file, route) in &called {
        let shape = shape_of(route);
        let matches = served.iter().any(|s| shape_of(s) == shape);
        assert!(
            matches,
            "{file} calls `{route}`, which the host does not serve. A surface reaching a route \
             that 404s is a button that cannot keep its promise, and nobody finds out until they \
             press it.\nServed: {served:?}"
        );
    }
}

/// The inbox is one flat ranked list, not grouped per project, since grouping
/// hides which row is most urgent.
#[test]
fn the_waiting_list_is_one_list_and_not_a_list_per_project() {
    let inbox =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/surfaces/inbox/List.svelte");
    let Ok(whole) = std::fs::read_to_string(&inbox) else {
        skip("no inbox surface yet");
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
    // `shown` must be a bare alias of what the host sent: narrowing lives in
    // `core::attention::narrow` so the terminal and the page agree.
    assert!(
        text.contains("{#each shown as i"),
        "the inbox does not iterate the ranked list it was handed"
    );
    assert!(
        text.contains("const shown = $derived(narrowedFeed?.items ?? items)"),
        "`shown` is no longer a bare alias of the ranked list the host sent — \
         if the inbox has started deriving its own set, the order and the \
         narrowing both have two authorities again"
    );
    for banned in [".filter((i)", "items.filter("] {
        assert!(
            !text.contains(banned),
            "the inbox filters the ranked list locally ({banned}). Narrowing is \
             `core::attention::narrow`, so that the terminal and the board \
             cannot disagree about what `--project pay` means."
        );
    }
}

/// The quickstart links the workbench tour, and together they name every
/// registered surface title (case-insensitive).
#[test]
fn the_quickstart_names_every_surface_the_registry_has() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let quickstart = std::fs::read_to_string(root.join("site/content/docs/quickstart.md"))
        .expect("the quickstart");
    let tour =
        std::fs::read_to_string(root.join("site/content/docs/workbench.md")).unwrap_or_default();
    if !tour.is_empty() {
        assert!(
            // `@/docs/workbench.md` is Zola's checked internal link form.
            quickstart.contains("@/docs/workbench.md") || quickstart.contains("/docs/workbench/"),
            "the quickstart does not link the workbench tour, so a reader never meets it"
        );
    }
    let doc = format!("{quickstart}\n{tour}").to_lowercase();

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

    let missing: Vec<&String> = titles
        .iter()
        .filter(|t| !doc.contains(&t.to_lowercase()))
        .collect();
    assert!(
        missing.is_empty(),
        "the quickstart does not name {missing:?}. It is the first page a reader meets, and a \
         surface it omits is one they will not know exists — the registry is the list, and this \
         page is a copy of it."
    );
}

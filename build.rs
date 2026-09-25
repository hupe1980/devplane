//! Embeds the built interface (`ui/dist`) into the binary at compile time.
//!
//! A missing bundle is a warning and `BUNDLE = None` (the binary serves a page
//! saying so), except in release or CI builds, where it is a hard failure.

use std::path::Path;

fn main() {
    // Only under the feature: a build without it must need nothing of Tauri's.
    #[cfg(feature = "app")]
    tauri_build::build();

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dist = root.join("ui/dist");

    // Rebuild when the bundle changes, and when it appears or disappears.
    println!("cargo:rerun-if-changed=ui/dist");
    println!("cargo:rerun-if-changed=build.rs");

    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let generated = out.join("ui_bundle.rs");

    let mut assets: Vec<(String, String)> = Vec::new();
    if dist.is_dir() {
        collect(&dist, &dist, &mut assets);
        assets.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let mut src = String::new();
    src.push_str(
        "/// One file of the built interface: the path it is served at, and the\n\
         /// bytes, embedded at compile time.\n\
         pub struct Asset {\n\
         \x20   pub path: &'static str,\n\
         \x20   pub bytes: &'static [u8],\n\
         }\n\n",
    );

    if assets.is_empty() {
        println!(
            "cargo:warning=ui/dist is absent: this binary will serve a page saying so instead of the interface (cd ui && npm run build)"
        );
        let release = std::env::var("PROFILE").is_ok_and(|p| p == "release");
        if release || std::env::var("CI").is_ok() {
            panic!(
                "ui/dist is absent in {} — build the interface first (cd ui && npm run build)",
                if release { "a release build" } else { "CI" }
            );
        }
        src.push_str(
            "/// No bundle was present when this was built. `None` rather than an\n\
             /// empty slice, because *nothing was built* and *a build produced\n\
             /// nothing* are different facts and only one of them is a bug.\n\
             pub const BUNDLE: Option<&[Asset]> = None;\n",
        );
    } else {
        src.push_str("pub const BUNDLE: Option<&[Asset]> = Some(&[\n");
        for (path, abs) in &assets {
            src.push_str(&format!(
                "    Asset {{ path: {path:?}, bytes: include_bytes!({abs:?}) }},\n"
            ));
        }
        src.push_str("]);\n");
    }

    std::fs::write(&generated, src).expect("writing the bundle index");
}

/// Every file under `dist`, as `(served path, absolute path)`. Source maps are
/// excluded: they would quadruple the binary for something nobody loads.
fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(base, &p, out);
        } else if p.extension().is_some_and(|x| x == "map") {
            continue;
        } else if p.is_file() {
            let rel = p
                .strip_prefix(base)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, p.to_string_lossy().to_string()));
        }
    }
}

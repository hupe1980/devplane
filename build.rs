//! Embeds the built interface into the binary at compile time.
//!
//! **One binary is the property this exists to keep.** The interface is built
//! by a second toolchain now, and the whole argument for accepting that was
//! that the *output* still ships inside one executable with nothing fetched and
//! nothing else to run. A bundle the binary loaded from disk at runtime would
//! give that away for nothing.
//!
//! # Why a missing bundle is not an error
//!
//! `cargo build` must work on a machine with no node, and it must work in a
//! clean checkout where `ui/dist/` has never existed — it is generated, and
//! therefore ignored. So this emits `BUNDLE: Option<&[Asset]>`, and the absence
//! is a value rather than a failure.
//!
//! What that must never become is a silent downgrade: a release binary that
//! quietly shipped without an interface because somebody forgot to build it.
//! The switch commit makes the bundle load-bearing, and
//! `tests/ui_contract.rs` fails when it is missing from then on. Until then the
//! binary serves `ui/legacy.html` and this is mechanism without a consumer,
//! which is deliberate: the interface is switched in **one** change, and
//! serving both at once is the risk this feature already refused.

use std::path::Path;

fn main() {
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

/// Every file under `dist`, as `(served path, absolute path)`.
///
/// Source maps are **excluded**. They are four times the size of the bundle,
/// they are a development aid rather than part of the interface, and embedding
/// them would quadruple the binary to ship something nobody loads over
/// loopback. The readable output is what makes the bundle followable without
/// one — which is the property the map would otherwise be compensating for.
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

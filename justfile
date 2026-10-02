# Devplane tasks. `check` is what CI runs; `verify` is everything a clean clone
# can run; `notes` needs the gitignored concepts/.

set shell := ["bash", "-uc"]

default:
    @just --list --unsorted

# ── Running ──────────────────────────────────────────────────────────────────

# The board in a browser. Starts the host if one is not already running.
open: build
    ./target/debug/devplane up

# The host serving ui/dist from disk: rebuild the bundle, reload. No cargo rebuild.
ui: build
    cd ui && npm run build
    DEVPLANE_UI=$PWD/ui/dist ./target/debug/devplane up --no-browser

# The host in the foreground.
serve: build
    ./target/debug/devplane up --no-browser

# Linux needs libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev.
# The desktop window: the host plus a tray item, notifications and a shortcut.
app:
    cargo run --features app -- up --window

# Any subcommand: `just vp config explain 'rm -rf /'`
vp *ARGS: build
    ./target/debug/devplane {{ARGS}}

# ── The loop ─────────────────────────────────────────────────────────────────

# `just --list` shows only the last comment line before a recipe.
# Everything CI runs, cheapest failure first — for this host.
check: fmt-check clippy build test wire-check

build:
    cargo build --all-targets --locked

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# The whole suite. Builds the ACP fixture agent first; plain `cargo test` cannot.
test: build
    cargo test --locked

# ── The checks that keep the claims honest ───────────────────────────────────

# Recompute the dependency figures the notes assert.
deps:
    bash scripts/deps-count.sh

# Every integration claim against its file in concepts/reference/. Run `just reference` first.
claims:
    bash scripts/verify-claims.sh

# Re-download the third-party reference docs into concepts/reference/ (gitignored).
reference:
    bash scripts/fetch-reference.sh

# concepts/ against its own rules: links resolve, decision ids unique, the published tree cites none of it.
concepts:
    bash scripts/concepts-check.sh

# Vendor changelog rows touching hooks and permission modes. Needs `just reference`.
# A report, not a check: exits 0 whatever it prints.
channels:
    bash scripts/changelog-rows.sh --channels

# concepts and claims — or `skipped: notes absent`.
notes:
    @if [ -d concepts ]; then bash scripts/concepts-check.sh; else echo "skipped: notes absent (concepts/ is gitignored and not in this checkout)"; fi
    @if [ -d concepts/reference ]; then bash scripts/verify-claims.sh; else echo "skipped: notes absent (concepts/reference/ is gitignored; \`just reference\` fetches it)"; fi

# Everything that runs on a clean clone: check, the dependency count, the site build and link check.
verify: check deps site-build site-check

# Needs a browser (`npx playwright-core install chromium`), so it is not in `verify`.
# The UX bars, each against its own budget: `just bars`, or `just bars --bar 3`.
bars *ARGS: build
    cd ui && npm run bars -- {{ARGS}}

# ── The two pictures ─────────────────────────────────────────────────────────

# The Open Graph card — what every shared link shows.
make-og:
    bash scripts/make-og.sh

# The board screenshot, from a throwaway host fed through the real endpoints.
make-board: build
    bash scripts/make-board.sh

# ── The site ─────────────────────────────────────────────────────────────────

# Live reload on http://127.0.0.1:1111
site:
    cd site && zola serve

site-build:
    cd site && zola build

# Fails on a broken internal link.
site-check:
    cd site && zola check

# ── Releasing ────────────────────────────────────────────────────────────────

# What a release would contain, without building it.
dist-plan:
    dist plan

# Would crates.io accept this manifest?
publish-check:
    cargo publish --dry-run --locked

clean:
    cargo clean
    rm -r site/public 2>/dev/null || true

# Regenerate the interface's wire types from the Rust shapes.
wire:
    TS_RS_EXPORT_DIR=ui/src cargo test --features typescript --quiet export_bindings

# The checked-in wire types equal what Rust generates: none missing, none drifted,
# none stale. CI runs the same diff; `just wire` fixes a failure. Kept out of
# `cargo test` because it compiles the crate a second time.
wire-check:
    #!/usr/bin/env bash
    set -euo pipefail
    tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
    TS_RS_EXPORT_DIR="$tmp" cargo test --locked --features typescript --quiet export_bindings >/dev/null
    diff -r ui/src/wire "$tmp/wire" || { echo "wire types drifted from the Rust shapes; run: just wire" >&2; exit 1; }

# Build the interface (needs node).
ui-build:
    cd ui && npm install && npm run build

# Type-check every component and every wire type.
ui-check:
    cd ui && npx svelte-check --tsconfig ./tsconfig.json

# Devplane tasks. `just` on its own lists them.
#
# `check` is exactly what .github/workflows/ci.yml runs; the rest are local,
# because they need concepts/, concepts/reference/ or a real agent binary.

set shell := ["bash", "-uc"]

default:
    @just --list --unsorted

# ── Running ──────────────────────────────────────────────────────────────────

# The board in a browser. Starts the daemon if one is not already running.
open: build
    ./target/debug/devplane open

# The same, serving ui/dist from disk: rebuild the bundle, reload. No cargo rebuild.
ui: build
    cd ui && npm run build
    DEVPLANE_UI=$PWD/ui/dist ./target/debug/devplane serve

# The daemon in the foreground.
serve: build
    ./target/debug/devplane serve

# Any subcommand: `just vp explain 'rm -rf /'`
vp *ARGS: build
    ./target/debug/devplane {{ARGS}}

# ── The loop ─────────────────────────────────────────────────────────────────

# NOTE: `just --list` prints only the LAST comment line of a recipe. Keep the
# summary on the final line — `check` used to advertise itself as "runs it on
# Linux, which is where a `cfg`-gated import goes unused unnoticed."
#
# CI also runs this on Linux, where a `cfg`-gated unused import shows up and a
# macOS-only run never would.
# Everything CI runs, cheapest failure first — for this host.
check: fmt-check clippy build test

build:
    cargo build --all-targets --locked

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# `cargo test` never rebuilds examples, so plain `cargo test` fails the
# conformance suite on a clean tree with "build the fixture first" — deliberate,
# since a silently stale fixture is worse than a loud missing one.
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


# concepts/ against its own rules: links resolve, D/R ids unique.
concepts:
    bash scripts/concepts-check.sh


# What the vendor changed about the **channels Devplane actually uses** — hooks,
# permission modes, the settings that decide whether a hook is consulted at all.
#
# The rule ledger that used to sit beside this is gone with the matcher it fed:
# Devplane no longer mirrors anybody's permission semantics, so a changed rule
# shape is the vendor's business. A changed *hook contract* is still ours.
channels:
    bash scripts/changelog-rows.sh --channels


# check, plus everything above that can run without a live agent.
verify: check deps concepts channels claims site-check

# ── The two pictures ─────────────────────────────────────────────────────────
#
# Both carry the product's name, so a rename invalidates them and nothing
# notices: no guard can read a PNG. Regenerating is a command rather than an
# afternoon in a design tool.

# The Open Graph card — what every shared link shows.
make-og:
    bash scripts/make-og.sh

# The board screenshot, from a throwaway daemon fed through the real endpoints.
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
#
# The TypeScript is generated, never written beside the Rust: a second copy of a
# wire format is a second thing to keep true, and the copy is the one that
# drifts. `tests/ui_bundle.rs` fails when what is checked in is not what this
# would produce.
wire:
    TS_RS_EXPORT_DIR=ui/src cargo test --features typescript --quiet export_bindings

# Build the interface. Needs node; `cargo build` works without it and serves
# the legacy page until the switch.
ui-build:
    cd ui && npm install && npm run build

# Type-check every component and every wire type.
ui-check:
    cd ui && npx svelte-check --tsconfig ./tsconfig.json

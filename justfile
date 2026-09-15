# Vibeplane tasks. `just` on its own lists them.
#
# `check` is exactly what .github/workflows/ci.yml runs; the rest are local,
# because they need concepts/, specs/ or a real agent binary.

set shell := ["bash", "-uc"]

default:
    @just --list --unsorted

# ── Running ──────────────────────────────────────────────────────────────────

# The board in a browser. Starts the daemon if one is not already running.
open: build
    ./target/debug/vibeplane open

# The daemon in the foreground.
serve: build
    ./target/debug/vibeplane serve

# Any subcommand: `just vp explain 'rm -rf /'`
vp *ARGS: build
    ./target/debug/vibeplane {{ARGS}}

# ── The loop ─────────────────────────────────────────────────────────────────

# Everything CI runs, cheapest failure first — but for this host only. CI also
# runs it on Linux, which is where a `cfg`-gated import goes unused unnoticed.
check: fmt-check clippy build test

build:
    cargo build --all-targets --locked

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Builds first: `cargo test` never rebuilds examples, and a stale fixture is refused.
test: build
    cargo test --locked

# ── The checks that keep the claims honest ───────────────────────────────────

# Recompute the dependency figures the notes assert.
deps:
    bash scripts/deps-count.sh

# Every integration claim against its file in specs/. Run `just specs` first.
claims:
    bash scripts/verify-claims.sh

# Re-download the third-party specs into specs/ (gitignored).
specs:
    bash scripts/fetch-specs.sh

# concepts/ against its own rules: links resolve, D/R ids unique.
concepts:
    bash scripts/concepts-check.sh

# Every CHANGELOG row that could change a verdict, against its ledger.
rows:
    bash scripts/changelog-rows.sh

# The rows not yet dispositioned, ready to paste into the ledger.
rows-new:
    bash scripts/changelog-rows.sh --new

# Both axes against a real `claude`. `just perms 20` caps it for a quick pass.
perms CASES="0": build
    bash scripts/verify-permissions-diff.sh {{CASES}}

# Allow axis only: does an `auto_allow` rule approve what Claude Code runs?
perms-allow CASES="0": build
    VIBEPLANE_DIFF_AXIS=allow bash scripts/verify-permissions-diff.sh {{CASES}}

# Deny axis only: does a `never_auto` rule stop what Claude Code refuses?
perms-deny CASES="0": build
    VIBEPLANE_DIFF_AXIS=deny bash scripts/verify-permissions-diff.sh {{CASES}}

# The written-down permission rules against a real Claude Code. Needs `claude`.
perms-live: build
    bash scripts/verify-permissions-live.sh

# check, plus everything above that can run without a live agent.
verify: check deps concepts rows claims site-check

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
    rm -rf site/public

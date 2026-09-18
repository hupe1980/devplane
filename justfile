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

# The same, serving ui/index.html from disk: edit, save, reload. No rebuild.
ui: build
    DEVPLANE_UI=$PWD/ui/index.html ./target/debug/devplane serve

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


# The span the measured floor could reach, and whether every probe still
# resolves. Spends nothing. The run that *does* spend is not a recipe, for the
# same reason publishing is not: a command that costs money is one somebody
# types on purpose, looking at it.
measured:
    bash scripts/measured-through.sh --dry-run

# concepts/ against its own rules: links resolve, D/R ids unique.
concepts:
    bash scripts/concepts-check.sh

# Every CHANGELOG row that could change a verdict, against its ledger.
# Prints how old the corpus is: a clean run over a stale one means very little.
rows:
    bash scripts/changelog-rows.sh

# The same, over a freshly downloaded changelog. One file, not all 130.
rows-fetch:
    bash scripts/changelog-rows.sh --fetch

# The rows not yet dispositioned, ready to paste into the ledger.
rows-new:
    bash scripts/changelog-rows.sh --new

# A cron line on the machine with the signed-in agent is the whole mechanism —
# no scheduler, no daemon, no service:
#
#   0 9 * * *  cd /path/to/devplane && just owed || notify "devplane: rows owed"
#
# The clock: non-zero when the vendor has shipped past the cleared floor.
owed:
    bash scripts/changelog-rows.sh --fetch >/dev/null
    bash scripts/changelog-rows.sh --owed

# The second ledger: the channels rather than the rules. Bounded on purpose —
# everything permission-adjacent is 220 rows, and a ledger nobody finishes looks
# like coverage.
channels:
    bash scripts/changelog-rows.sh --channels

# The expensive floor (`VERIFIED_AGAINST`) is not touched here: it moves only
# when `just perms` runs green, which costs a signed-in agent and real money.
#
# Moves the cleared-rows floor to the vendor's head, only on a green ledger.
advance:
    bash scripts/changelog-rows.sh --advance

# Both axes against a real `claude`. `just perms 20` caps it for a quick pass.
perms CASES="0": build
    bash scripts/verify-permissions-diff.sh {{CASES}}

# Allow axis only: does an `auto_allow` rule approve what Claude Code runs?
perms-allow CASES="0": build
    DEVPLANE_DIFF_AXIS=allow bash scripts/verify-permissions-diff.sh {{CASES}}

# Deny axis only: does a `never_auto` rule stop what Claude Code refuses?
perms-deny CASES="0": build
    DEVPLANE_DIFF_AXIS=deny bash scripts/verify-permissions-diff.sh {{CASES}}

# One rule set, both axes, so a disagreement is re-asked cheaply: `just perms-only 'Read(.env)'`
perms-only RULE: build
    DEVPLANE_DIFF_ONLY='{{RULE}}' bash scripts/verify-permissions-diff.sh

# PowerShell, Monitor and LSP answered by this matcher alone: a checklist, not a measurement.
perms-dialect: build
    DEVPLANE_DIFF_AXIS=dialect bash scripts/verify-permissions-diff.sh

# Are both probes handed the same rules? The one question the harness has no oracle for.
perms-selftest:
    DEVPLANE_DIFF_AXIS=selftest bash scripts/verify-permissions-diff.sh

# The written-down permission rules against a real Claude Code. Needs `claude`.
perms-live: build
    bash scripts/verify-permissions-live.sh

# check, plus everything above that can run without a live agent.
verify: check deps concepts rows claims site-check perms-selftest

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
    rm -rf site/public

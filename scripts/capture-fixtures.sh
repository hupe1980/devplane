#!/usr/bin/env bash
# Records what a real, seeded host serves, as the interface's design fixtures.
#
#   bash scripts/capture-fixtures.sh                        # writes ui/fixtures/*.json
#   bash scripts/capture-fixtures.sh --phase empty          # ui/fixtures/phases/empty/
#   bash scripts/capture-fixtures.sh --phase nothing-visible
#   cd ui && npm run dev:fixtures                           # the interface over those files
#
# A phase is the same routes read from a bare host: `empty` has seen one session
# start and end and holds nothing; `nothing-visible` is a fresh home no channel
# has reported to. The bars read them to tell absence from zero. `OUT` records
# somewhere else (to compare before replacing).
#
# Recorded from a host seeded by `seed-host.sh`, never hand-written; re-record rather
# than edit. Paths are rewritten to a short fixed root.
set -eu
cd "$(dirname "$0")/.."
SEED_PHASE=""
if [ "${1:-}" = --phase ]; then
  SEED_PHASE="${2:?capture-fixtures: --phase empty|nothing-visible}"
fi
export SEED_PHASE
BIN="${BIN:-./target/debug/devplane}"
[ -x "$BIN" ] || { echo "capture-fixtures: build first (cargo build --examples)" >&2; exit 2; }
SHOT_DIR="${SHOT_DIR:-${TMPDIR:-/tmp}/devplane-fixtures}"
. scripts/seed-host.sh

out="${OUT:-ui/fixtures${SEED_PHASE:+/phases/$SEED_PHASE}}"
mkdir -p "$out"
# The recordings at this level only: the phase sets below it are their own runs.
find "$out" -maxdepth 1 -name '*.json' -exec rm {} +
# Mask the real spelling (`/private/tmp` on macOS) before the short one.
real=$(cd "$tmp" && pwd -P)
# A path the host shortened for display ends in `…` before the fixture root is
# whole; mask what is left of it, or a long scratch directory leaks through.
clip() {
  R="$real" T="$tmp" perl -CSD -pe 'for my $p ($ENV{R}, $ENV{T}) {
    for (my $n = length($p) - 1; $n >= 8; $n--) {
      my $q = quotemeta(substr($p, 0, $n)); s#$q\x{2026}#/home/you/code/\x{2026}#g } }'
}
# A `BIN` built elsewhere (another target directory) reads as the default one.
bindir=$(dirname "$BIN")
save() { # route file
  api GET "$1" | sed -e "s#$bindir#/opt/devplane/target/debug#g" -e "s#$real#/home/you/code#g" -e "s#$tmp#/home/you/code#g" -e "s#$PWD#/opt/devplane#g" | clip > "$out/$2.json"
}
for r in board inbox changes decisions projects specs setup forge agents quitting first-run; do
  save "api/$r" "$r" || echo "capture-fixtures: /api/$r did not answer" >&2
done
save "api/search?q=Read" search || true
# Every change with its review and certificate, every run with its transcript.
for id in $(api GET api/changes | grep -oE '"id": *"c-[^"]+"' | grep -oE 'c-[^"]+'); do
  save "api/changes/$id" "change-$id"
  save "api/changes/$id/review" "review-$id" || true
  save "api/changes/$id/certificate" "certificate-$id" || true
  # An offer the host refuses is recorded too, so the window's refusal can be
  # seen without a host. Asked only of a change whose review leads with a
  # weakened check nobody has read, so it is always refused, never made.
  grep -q '"checks weakened or changed"' "$out/review-$id.json" 2>/dev/null || continue
  refused=$(curl -sS -X POST "http://127.0.0.1:$port/api/changes/$id/offer" \
    -H "Authorization: Bearer $token" -H 'content-type: application/json' -d '{}' || true)
  case "$refused" in
    *'"refused"'*) printf '%s' "$refused" | sed -e "s#$real#/home/you/code#g" -e "s#$tmp#/home/you/code#g" | clip > "$out/offer-$id.json" ;;
  esac
done
for id in $(api GET api/board | grep -oE '"id": *"(acp|cc|run)-[^"]+"' | grep -oE '(acp|cc|run)-[^"]+' | sort -u); do
  save "api/runs/$id" "run-$id" || true
  save "api/runs/$id/messages" "messages-$id" || true
done
# A phase set must be the phase it claims: `empty` has seen a session, and
# `nothing-visible` none. (`watching` is the vendor table, the same on every
# host, so the sessions seen are what tells the two apart.)
if [ -n "$SEED_PHASE" ]; then
  seen=$(node -e 'const b=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")); console.log((b.runs ?? []).length)' "$out/board.json")
  case "$SEED_PHASE:$seen" in
    empty:0) echo "capture-fixtures: the empty host saw no session; it should have seen one" >&2; exit 1 ;;
    nothing-visible:0) ;;
    nothing-visible:*) echo "capture-fixtures: the fresh host saw $seen sessions; it should see none" >&2; exit 1 ;;
  esac
fi
echo "capture-fixtures: $(find "$out" -maxdepth 1 -name '*.json' | wc -l | tr -d ' ') responses in $out"

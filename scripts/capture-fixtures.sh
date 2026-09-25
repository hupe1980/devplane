#!/usr/bin/env bash
# Records what a real, seeded host serves, as the interface's design fixtures.
#
#   bash scripts/capture-fixtures.sh        # writes ui/fixtures/*.json
#   cd ui && npm run dev:fixtures           # the interface over those files
#
# Recorded from a host seeded by `seed-host.sh`, never hand-written; re-record rather
# than edit. Paths are rewritten to a short fixed root.
set -eu
cd "$(dirname "$0")/.."
BIN=./target/debug/devplane
[ -x "$BIN" ] || { echo "capture-fixtures: build first (cargo build --examples)" >&2; exit 2; }
SHOT_DIR="${SHOT_DIR:-${TMPDIR:-/tmp}/devplane-fixtures}"
. scripts/seed-host.sh

out=ui/fixtures
rm -r "$out" 2>/dev/null || true
mkdir -p "$out"
# Mask the real spelling (`/private/tmp` on macOS) before the short one.
real=$(cd "$tmp" && pwd -P)
save() { # route file
  api GET "$1" | sed -e "s#$real#/home/you/code#g" -e "s#$tmp#/home/you/code#g" -e "s#$PWD#/opt/devplane#g" > "$out/$2.json"
}
for r in board inbox changes decisions reports projects specs setup forge agents quitting; do
  save "api/$r" "$r" || echo "capture-fixtures: /api/$r did not answer" >&2
done
save "api/search?q=Read" search || true
# Every change with its review and certificate, every run with its transcript.
for id in $(api GET api/changes | grep -oE '"id": *"c-[^"]+"' | grep -oE 'c-[^"]+'); do
  save "api/changes/$id" "change-$id"
  save "api/changes/$id/review" "review-$id" || true
  save "api/changes/$id/certificate" "certificate-$id" || true
done
for id in $(api GET api/board | grep -oE '"id": *"(acp|cc|run)-[^"]+"' | grep -oE '(acp|cc|run)-[^"]+' | sort -u); do
  save "api/runs/$id" "run-$id" || true
  save "api/runs/$id/messages" "messages-$id" || true
done
echo "capture-fixtures: $(ls "$out" | wc -l | tr -d ' ') responses in $out"

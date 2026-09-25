#!/usr/bin/env bash
# Renders site/static/og.png from scripts/og-card.html.
# Needs Chrome. 1200×630, Open Graph's size.
#
#   bash scripts/make-og.sh
set -eu
cd "$(dirname "$0")/.."

CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
[ -x "$CHROME" ] || { echo "make-og: no Chrome at $CHROME — set CHROME=" >&2; exit 2; }

out=$(mktemp -d)
# `rm -r` rather than `rm -rf`: this repository prohibits the force form.
trap 'rm -r "$out" 2>/dev/null || true' EXIT
"$CHROME" --headless --disable-gpu --hide-scrollbars \
  --window-size=1200,630 --screenshot="$out/og.png" \
  --default-background-color=00000000 \
  "file://$PWD/scripts/og-card.html" >/dev/null 2>&1

[ -s "$out/og.png" ] || { echo "make-og: Chrome produced nothing" >&2; exit 1; }
mv "$out/og.png" site/static/og.png
echo "make-og: site/static/og.png rewritten ($(du -k site/static/og.png | cut -f1) KB)"

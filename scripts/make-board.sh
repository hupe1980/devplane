#!/usr/bin/env bash
# Renders the site's screenshots from a real host fed real payloads.
# A throwaway host on its own `DEVPLANE_HOME`, seeded through the real endpoints
# and photographed. Needs Chrome and a debug build.
#
#   bash scripts/make-board.sh
set -eu
cd "$(dirname "$0")/.."

CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
BIN=./target/debug/devplane
[ -x "$CHROME" ] || { echo "make-board: no Chrome at $CHROME — set CHROME=" >&2; exit 2; }
[ -x "$BIN" ]    || { echo "make-board: build first (just build)" >&2; exit 2; }

# The interface is embedded in the binary, so the binary must be newer than
# `ui/dist/`; `just make-board` rebuilds both.
if [ ! -f ui/dist/index.html ]; then
  echo "make-board: no ui/dist — run 'cd ui && npm run build', then rebuild" >&2
  exit 2
fi
if [ ui/dist/index.html -nt "$BIN" ]; then
  echo "make-board: ui/dist is newer than $BIN — run 'just make-board'" >&2
  exit 2
fi

# Shared with anything else that photographs the interface.
. "$(dirname "$0")/seed-host.sh"

# One picture per surface, addressed by hash so no script is injected. No
# `--virtual-time-budget`: the open SSE stream would make Chrome wait for ever.
shoot() { # file-name  hash  height  [light|dark]  [width]  [touch]
  local name="$1" hash="$2" height="${3:-940}" scheme="${4:-}" width="${5:-1240}" touch="${6:-}"
  # Headless Chrome reports dark and a mouse; set the scheme and a coarse pointer
  # (`primaryPointerType=2`) explicitly so the light and phone shots test their rules.
  local blink=()
  # Blink's numbering: 0 is dark, 1 is light.
  [ "$scheme" = light ] && blink+=(preferredColorScheme=1)
  [ "$scheme" = dark ] && blink+=(preferredColorScheme=0)
  [ -n "$touch" ] && blink+=(primaryPointerType=2 availablePointerTypes=2)
  local flags=()
  [ ${#blink[@]} -gt 0 ] && flags+=(--blink-settings="$(IFS=,; echo "${blink[*]}")")
  "$CHROME" --headless --disable-gpu --hide-scrollbars "${flags[@]:+${flags[@]}}" \
    --window-size="$width,$height" --screenshot="$tmp/$name.png" \
    "http://127.0.0.1:$port/?token=$token#$hash" >/dev/null 2>&1 || true
  [ -s "$tmp/$name.png" ] || { echo "make-board: Chrome produced nothing for $name" >&2; return 1; }
  mv "$tmp/$name.png" "site/static/$name.png"
  echo "make-board: site/static/$name.png ($(du -k "site/static/$name.png" | cut -f1) KB)"
}

# The change the seed verified, and its review — which leads with the test the
# seed's agent skipped. Found through the host rather than guessed.
change=$(printf '%s' "$rate" | grep -oE '"change_id": *"c-[0-9a-f]+"' | head -1 | grep -oE 'c-[0-9a-f]+')
[ -n "$change" ] || { echo "make-board: the seed has no saas change" >&2; exit 1; }

# The workbench at laptop size: a change, its review, the sessions,
# what needs you, and the ledger.
shoot workbench "change/$change" 900 dark 1440
shoot review "review/$change" 900 dark 1440
shoot sessions board 820 dark 1440
[ "${SHOTS:-all}" = board ] || {
  shoot inbox inbox 760 dark 1440
  shoot audit why 700 dark 1440
}

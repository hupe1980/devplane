#!/usr/bin/env bash
# Renders the site's screenshots from a real daemon fed real payloads.
#
# The screenshot on the README and the landing page carries the product's name
# in its header, so a rename invalidates it and no check can read it: it said
# "VIBEPLANE" for as long as nobody looked at the picture.
#
# **It is not a mock.** A throwaway daemon is started on its own
# `DEVPLANE_HOME`, seeded through the same hook and status-line endpoints Claude
# Code posts to, and photographed. Whatever the board does with that data is
# what ends up in the image — which is the only way a screenshot stays honest.
#
#   bash scripts/make-board.sh
#
# Needs Chrome and a debug build (`just build`).
set -eu
cd "$(dirname "$0")/.."

CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
BIN=./target/debug/devplane
[ -x "$CHROME" ] || { echo "make-board: no Chrome at $CHROME — set CHROME=" >&2; exit 2; }
[ -x "$BIN" ]    || { echo "make-board: build first (just build)" >&2; exit 2; }

# **The interface is compiled into the binary**, so an edit to `ui/` is not in
# the picture until `npm run build` *and* a rebuild of the binary. Running this
# script directly rather than through `just make-board` skips both, and the
# shots come back showing the old interface while reporting success — which cost
# two rounds of "the CSS does not work" before anybody checked what was being
# served.
#
# The check is against the built bundle rather than the sources: `ui/dist/` is
# what `build.rs` embeds, so that is the file whose age decides whether the
# binary is current.
if [ ! -f ui/dist/index.html ]; then
  echo "make-board: no ui/dist — run 'cd ui && npm run build', then rebuild" >&2
  exit 2
fi
if [ ui/dist/index.html -nt "$BIN" ]; then
  echo "make-board: ui/dist is newer than $BIN — run 'just make-board'" >&2
  exit 2
fi

# The fixture and the daemon, shared with `shoot-rebuild.sh` so the two
# interfaces are photographed over identical data. See `scripts/seed-daemon.sh`.
. "$(dirname "$0")/seed-daemon.sh"

# One picture per surface, from the same seeded daemon.
#
# The board is not the only thing worth showing any more. **Gate is the surface
# nobody else in the field ships**, and a README that shows only a session list
# is a README about the half that commoditised — four watchers do that, and one
# of them has ten times the stars.
#
# Each shot drives the rail the way a person would, rather than loading a
# different address: the rail is chrome, so there is no URL to photograph. The
# theme is set the same way the toggle sets it, which is also a check — if the
# attribute stopped working, the light shot would come back dark.
# A surface is addressed by its hash, so a picture of one needs no script
# injected into the page — the rail's links are ordinary links and the hash is
# what decides.
#
# **No `--virtual-time-budget`.** The board holds an SSE stream open, so virtual
# time never advances past it and Chrome waits for ever. That is not a
# hypothetical: it is how this script first hung.
shoot() { # file-name  hash  height  [light|dark]  [width]  [touch]
  local name="$1" hash="$2" height="${3:-940}" scheme="${4:-}" width="${5:-1240}" touch="${6:-}"
  # Headless Chrome answers `prefers-color-scheme: dark`, so the default shots
  # are dark. `preferredColorScheme` drives the media query directly, which is
  # the only way to photograph the other theme without injecting a script —
  # and it means the light shot is a real test of the light tokens rather than
  # a picture of the same page.
  #
  # **And `pointer: coarse` for the phone shot.** Headless Chrome reports a
  # mouse, so the media query that hides the key legend on a touch device never
  # fired and the narrow picture came back identical \u2014 the rule was written,
  # shipped, and photographed as not working, which is R49 again in a new place.
  # `primaryPointerType=2` is `POINTER_TYPE_COARSE`; the shot is now a test of
  # the rule rather than a hope about it.
  local blink=()
  [ "$scheme" = light ] && blink+=(preferredColorScheme=1)
  [ "$scheme" = dark ] && blink+=(preferredColorScheme=2)
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

# **The narrow view, at 500 — which is as narrow as Chrome will go.**
#
# `--window-size` is clamped: asking for 390 gives a 500-point *layout* cropped
# to a 390-wide PNG, which looks exactly like a page that scrolls sideways and
# is not one. A whole phone-overflow "bug" was chased before a diagnostic
# printed `vw500 sw500` and settled it — the page had never overflowed, and
# `--headless=new` clamps the same way.
#
# 500 still exercises the narrow layout, because the media query turns over at
# 46rem. What it does not prove is 390, and saying 390 when the tool gives 500
# would be the kind of claim this project fails builds over.
# No companion shot at 390. Asking for it produces a 500-point layout cropped to
# a 390-wide PNG — a picture of Chrome's clamping rather than of the page, and
# publishing one next to the real narrow shot is how the phantom bug started.
shoot narrow inbox 900 "" 500 touch

shoot board board 940
# Straight after the dark one, so the two are the same board seconds apart and
# a reader comparing them is comparing themes rather than fixture ages. Taken
# last, the inbox item had aged off and the light shot showed no "needs you" —
# which reads as a missing feature rather than as a stale screenshot.
shoot board-light board 940 light
[ "${SHOTS:-all}" = board ] || {
  # Gate is the surface nobody else in the field ships, and a README showing
  # only a session list is a README about the half that commoditised.
  shoot audit why   820
  shoot inbox inbox 620
  shoot work  work  420
}

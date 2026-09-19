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

# **The page is compiled into the binary**, so an edit to `ui/index.html` is not
# in the picture until a rebuild. Running this script directly rather than
# through `just make-board` skips that, and the shots come back showing the old
# page while reporting success — which cost two rounds of "the CSS does not
# work" before anybody checked which page was being served.
if [ ui/index.html -nt "$BIN" ]; then
  echo "make-board: ui/index.html is newer than $BIN — run 'just make-board'" >&2
  exit 2
fi

# A short, fixed base: the offer names the project's own settings file by
# absolute path, and a `mktemp` path would put forty characters of
# `/var/folders/_6/...` into the picture.
#
# **The path is fixed, so `mkdir` is the lock.** Two runs at once used to share
# it, and each one's cleanup deleted the other's fixture out from under Chrome:
# the second run reported "Chrome produced nothing" for some shots, wrote the
# rest, and exited 0. A partial set of screenshots that claims success is worse
# than none, because the ones it did write look fine.
tmp=/tmp/devplane-shot
home="$tmp/home"
if ! mkdir "$tmp" 2>/dev/null; then
  echo "make-board: $tmp exists — another run has it, or one died. rm -rf it." >&2
  exit 2
fi
mkdir -p "$home"
cleanup() {
  local status=$?
  [ -n "${pid:-}" ] && kill "$pid" 2>/dev/null || true
  rm -rf "$tmp"
  # The trap must not launder a failure into a success. `shoot` returns
  # non-zero when Chrome writes nothing, and that has to reach the caller.
  exit "$status"
}
trap cleanup EXIT

# Five repositories, because the board groups by project and a screenshot of one
# project shows none of that.
for name in saas core-lib ai-tool mobile infra; do
  mkdir -p "$tmp/$name"
  git -C "$tmp/$name" init -q
done

# **And an empty Claude Code config.** Without this the daemon does what it is
# built to do — discovers the real sessions on this machine from Claude Code's
# own roster — and the marketing screenshot fills up with somebody's actual
# project names. Caught by looking at the picture, which is the only way.
mkdir -p "$tmp/claude/projects"
CLAUDE_CONFIG_DIR="$tmp/claude" DEVPLANE_HOME="$home" "$BIN" serve >"$tmp/daemon.log" 2>&1 &
pid=$!
for _ in $(seq 1 50); do [ -s "$home/daemon.json" ] && break; sleep 0.2; done
[ -s "$home/daemon.json" ] || { echo "make-board: the daemon did not start"; cat "$tmp/daemon.log"; exit 1; }
port=$(grep -oE '"port": *[0-9]+' "$home/daemon.json" | grep -oE '[0-9]+')
token=$(cat "$home/token")

post() { curl -sS -X POST "http://127.0.0.1:$port/$1" -H "Authorization: Bearer $token" \
  -H 'content-type: application/json' -d "$2" >/dev/null; }

# A session, as Claude Code reports one: the tool call it is making, then the
# status line that carries cost, context and the release it runs.
session() { # id project tool input pct cost
  post devplane/hook "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"$1\",
    \"cwd\":\"$tmp/$2\",\"tool_name\":\"$3\",\"tool_input\":$4}"
  post devplane/statusline "{\"session_id\":\"$1\",\"model\":{\"display_name\":\"Opus 5\"},
    \"context_window\":{\"used_percentage\":$5,\"context_window_size\":200000},
    \"cost\":{\"total_cost_usd\":$6},\"workspace\":{\"current_dir\":\"$tmp/$2\"}}"
}

session 2b3c4d5e saas     Bash '{"command":"cargo test --workspace"}' 62 1.92
session 7c3a1f00 saas     Edit '{"file_path":"src/routes/login.ts"}'   67 1.04
session c19d02aa saas     Read '{"file_path":"src/routes/session.ts"}' 41 0.38
session 4f5e6d7c core-lib Bash '{"command":"pnpm build"}'              3  0.02
session e8112b40 core-lib Read '{"file_path":"src/core/policy.rs"}'    58 0.77
session 9e8d7c6b ai-tool  Edit '{"file_path":"src/retry.rs"}'          46 0.67
session 31c9ae05 infra    Edit '{"file_path":"terraform/main.tf"}'     55 2.40
session b7710ff2 mobile   Bash '{"command":"git push origin HEAD"}'    72 0.08

# One permission nobody has a rule for — the item the board exists to surface,
# and now the one carrying the rule that would end it. Three calls of its family
# first, because the wide rule is offered only where the evidence is.
for c in "pnpm test --run" "pnpm test --watch" "pnpm test -u"; do
  post devplane/hook "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"04ab12cd\",
    \"cwd\":\"$tmp/mobile\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"$c\"}}"
done
post devplane/decided "{\"session\":\"04ab12cd\",\"verdict\":\"undecided\",\"rule\":null,
  \"blocked\":true,\"subject\":\"Bash: pnpm test --coverage\",\"tool\":\"Bash\",
  \"payload\":{\"hook_event_name\":\"PermissionRequest\",\"session_id\":\"04ab12cd\",
  \"cwd\":\"$tmp/mobile\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"pnpm test --coverage\"}}}"
post devplane/statusline '{"session_id":"04ab12cd","model":{"display_name":"Opus 5"},
  "context_window":{"used_percentage":74,"context_window_size":200000},
  "cost":{"total_cost_usd":0.90}}'

# And a context window close to compaction, which is the other thing worth seeing.
post devplane/hook "{\"hook_event_name\":\"Notification\",\"session_id\":\"a1b2c3d4\",
  \"cwd\":\"$tmp/core-lib\",\"notification_type\":\"idle_prompt\"}"
post devplane/statusline '{"session_id":"a1b2c3d4","model":{"display_name":"Opus 5"},
  "context_window":{"used_percentage":89,"context_window_size":200000},
  "cost":{"total_cost_usd":0.41}}'

sleep 1

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
shoot narrow needs 900 "" 500 touch

shoot board boardsec 940
# Straight after the dark one, so the two are the same board seconds apart and
# a reader comparing them is comparing themes rather than fixture ages. Taken
# last, the inbox item had aged off and the light shot showed no "needs you" —
# which reads as a missing feature rather than as a stale screenshot.
shoot board-light boardsec 940 light
[ "${SHOTS:-all}" = board ] || {
  # Gate is the surface nobody else in the field ships, and a README showing
  # only a session list is a README about the half that commoditised.
  shoot audit auditsec 820
  shoot inbox needs    620
  shoot work  worksec  420
}

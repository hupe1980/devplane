#!/usr/bin/env bash
# Renders site/static/board.png from a real daemon fed real payloads.
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

# A short, fixed base: the offer names the project's own `devplane.toml` by
# absolute path, and a `mktemp` path would put forty characters of
# `/var/folders/_6/...` into the picture.
tmp=/tmp/devplane-shot
rm -rf "$tmp"
home="$tmp/home"
mkdir -p "$home"
cleanup() {
  [ -n "${pid:-}" ] && kill "$pid" 2>/dev/null || true
  rm -rf "$tmp"
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
"$CHROME" --headless --disable-gpu --hide-scrollbars \
  --window-size=1240,940 --screenshot="$tmp/board.png" \
  "http://127.0.0.1:$port/?token=$token" >/dev/null 2>&1

[ -s "$tmp/board.png" ] || { echo "make-board: Chrome produced nothing" >&2; exit 1; }
mv "$tmp/board.png" site/static/board.png
echo "make-board: site/static/board.png rewritten ($(du -k site/static/board.png | cut -f1) KB)"

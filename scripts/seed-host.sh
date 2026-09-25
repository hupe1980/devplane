#!/usr/bin/env bash
# One seeded host, shared by everything that photographs or records an interface.
# Sourced, never run: leaves `$tmp`, `$home`, `$pid`, `$port` and `$token` set.
#
#   BIN=./target/debug/devplane SHOT_DIR=/tmp/devplane-shot
#   . scripts/seed-host.sh
#
# Sessions arrive as real `devplane hook`/`statusline` processes; changes are driven
# over the host's API by the ACP fixture agent, so their git state and gates are real.
# Requests fail loudly.

: "${BIN:?seed-host: set BIN to a built devplane}"
# Absolute, because the host is started from inside the fixture (below).
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
tmp="${SHOT_DIR:-/tmp/devplane-shot}"
home="$tmp/home"
ECHO="${ECHO_AGENT:-$(dirname "$BIN")/examples/echo_agent}"
[ -x "$ECHO" ] || { echo "seed-host: no fixture agent at $ECHO — cargo build --examples" >&2; exit 2; }

# `mkdir` is the lock: concurrent runs would delete each other's fixture.
if ! mkdir "$tmp" 2>/dev/null; then
  echo "seed-host: $tmp exists — another run has it, or one died. Delete it." >&2
  exit 2
fi
mkdir -p "$home"
cleanup() {
  local status=$?
  [ -n "${pid:-}" ] && kill "$pid" 2>/dev/null || true
  # `rm -r`, not `rm -rf`: this repository prohibits `Bash(rm -rf *)`.
  chmod -R u+w "$tmp" 2>/dev/null || true
  rm -r "$tmp" 2>/dev/null || true
  exit "$status"
}
trap cleanup EXIT

# An empty Claude Code config, so the host does not discover this machine's real sessions.
mkdir -p "$tmp/claude/projects"
export CLAUDE_CONFIG_DIR="$tmp/claude" DEVPLANE_HOME="$home"
export GIT_AUTHOR_NAME=Seed GIT_AUTHOR_EMAIL=seed@example.com
export GIT_COMMITTER_NAME=Seed GIT_COMMITTER_EMAIL=seed@example.com

# Five repositories, because the board groups by project. `saas` is a Spec Kit
# project with a real specification and real gates; the rest are watched only.
for name in saas core-lib ai-tool mobile infra; do
  mkdir -p "$tmp/$name"
  git -C "$tmp/$name" init -q -b main
  # The fixture agent's own log must not show up in a review.
  printf '.devplane/\n' > "$tmp/$name/.gitignore"
done
(
  cd "$tmp/saas"
  mkdir -p .specify src/routes tests specs/042-login-rate-limit specs/043-drop-v1-api
  cat > devplane.toml <<'TOML'
[project]
name = "saas"

[gates]
check = ["sh ./scripts/lint.sh", "sh ./scripts/test.sh"]
timeout = "60s"

[policy]
never_auto = ["Read(.env)", "Bash(rm -rf *)"]
always_ask = ["Bash(git push *)"]

[review.roles]
shared   = ["src/types/**", "migrations/**"]
security = ["src/auth/**"]
wiring   = ["src/routes/**"]
tests    = ["tests/**"]

[[review.covers]]
test  = "tests/login.test.ts"
paths = ["src/routes/login.ts", "src/auth/**"]
TOML
  mkdir -p scripts
  printf '#!/bin/sh\necho "lint: 14 files, 0 problems"\n' > scripts/lint.sh
  printf '#!/bin/sh\necho "PASS tests/login.test.ts (6)"\necho "PASS tests/session.test.ts (11)"\n' > scripts/test.sh
  printf 'export function login() {\n  return true;\n}\n' > src/routes/login.ts
  printf 'test("login", () => {});\n' > tests/login.test.ts
  cat > specs/042-login-rate-limit/spec.md <<'MD'
# Feature Specification: Rate-limit the login route

## User Scenarios

### User Story 1 - A burst of failed logins is slowed (Priority: P1)

### User Story 2 - A person locked out is told when to retry (Priority: P2)

## Requirements

- **FR-001** The login route MUST reject a sixth failed attempt within a minute.
- **FR-002** A rejected attempt MUST say when the next attempt is accepted.
- **FR-003** A successful login MUST reset the counter. [NEEDS CLARIFICATION: per account or per address?]
MD
  cat > specs/042-login-rate-limit/tasks.md <<'MD'
# Tasks: Rate-limit the login route

## Phase 1: User Story 1

- [x] T001 [US1] Add a sliding-window counter in src/auth/limit.ts
- [x] T002 [US1] Reject the sixth attempt in src/routes/login.ts
- [ ] T003 [US1] Test the burst in tests/login.test.ts

## Phase 2: User Story 2

- [ ] T004 [US2] Return Retry-After in src/routes/login.ts
- [ ] T005 [US2] Render the retry time in the login form
MD
  printf '# Feature Specification: Drop the v1 API\n\n### User Story 1 - v1 routes answer 410 (Priority: P1)\n' > specs/043-drop-v1-api/spec.md
  printf '# Tasks\n\n- [ ] T001 [US1] Return 410 from every v1 route\n- [ ] T002 [US1] Remove the v1 client\n' > specs/043-drop-v1-api/tasks.md
  git add -A && git commit -qm "the login route, its gates and two specifications"
)
for name in core-lib ai-tool mobile infra; do
  printf '# %s\n' "$name" > "$tmp/$name/README.md"
  git -C "$tmp/$name" add -A && git -C "$tmp/$name" commit -qm init
done

# Started from inside the fixture, because the host registers its start directory.
(cd "$tmp" && exec "$BIN" serve) >"$tmp/host.log" 2>&1 &
pid=$!
for _ in $(seq 1 50); do [ -s "$home/host.json" ] && break; sleep 0.2; done
[ -s "$home/host.json" ] || { echo "seed-host: the host did not start"; cat "$tmp/host.log"; exit 1; }
port=$(grep -oE '"port": *[0-9]+' "$home/host.json" | grep -oE '[0-9]+')
token=$(cat "$home/token")

api() { # method path [json] — prints the body, and the body again on stderr when it failed
  local out
  if ! out=$(curl --fail-with-body -sS -X "$1" "http://127.0.0.1:$port/$2" \
      -H "Authorization: Bearer $token" -H 'content-type: application/json' ${3:+-d "$3"}); then
    echo "seed-host: $1 /$2 failed: $out" >&2
    return 1
  fi
  printf "%s" "$out"
}

# A watched session: a hook process, then the status-line shim, each with its payload on stdin.
hook() { printf '%s' "$1" | "$BIN" hook >/dev/null; }
statusline() { printf '%s' "$1" | "$BIN" statusline >/dev/null; }
session() { # id project tool input pct cost
  hook "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"$1\",\"cwd\":\"$tmp/$2\",\"source\":\"startup\"}"
  hook "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"$1\",\"cwd\":\"$tmp/$2\",\"tool_name\":\"$3\",\"tool_input\":$4}"
  statusline "{\"session_id\":\"$1\",\"model\":{\"display_name\":\"Opus 5\"},
    \"context_window\":{\"used_percentage\":$5,\"context_window_size\":200000},
    \"cost\":{\"total_cost_usd\":$6},\"workspace\":{\"current_dir\":\"$tmp/$2\"}}"
}

session 2b3c4d5e saas     Bash '{"command":"pnpm test --run"}'             62 1.92
session c19d02aa saas     Read '{"file_path":"src/routes/session.ts"}'     41 0.38
session 4f5e6d7c core-lib Bash '{"command":"cargo build --release"}'       3  0.02
session e8112b40 core-lib Read '{"file_path":"src/core/policy.rs"}'        58 0.77
session 9e8d7c6b ai-tool  Edit '{"file_path":"src/retry.rs"}'              46 0.67
session 31c9ae05 infra    Edit '{"file_path":"terraform/main.tf"}'         55 2.40

# A prohibition a rule enforced, with nobody asked.
hook "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"2b3c4d5e\",\"cwd\":\"$tmp/saas\",
  \"tool_name\":\"Read\",\"tool_input\":{\"file_path\":\"$tmp/saas/.env\"}}"

# A permission waiting on a person — the item the inbox exists to surface.
hook "{\"hook_event_name\":\"PermissionRequest\",\"session_id\":\"b7710ff2\",\"cwd\":\"$tmp/mobile\",
  \"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"git push origin HEAD\"}}" &
statusline "{\"session_id\":\"b7710ff2\",\"model\":{\"display_name\":\"Opus 5\"},
  \"context_window\":{\"used_percentage\":74,\"context_window_size\":200000},\"cost\":{\"total_cost_usd\":0.90}}"

# A session close to compaction, idle and waiting for its next prompt.
hook "{\"hook_event_name\":\"Notification\",\"session_id\":\"a1b2c3d4\",\"cwd\":\"$tmp/core-lib\",
  \"notification_type\":\"idle_prompt\",\"message\":\"Claude is waiting for your input\"}"
statusline "{\"session_id\":\"a1b2c3d4\",\"model\":{\"display_name\":\"Opus 5\"},
  \"context_window\":{\"used_percentage\":89,\"context_window_size\":200000},\"cost\":{\"total_cost_usd\":0.41}}"

# Changes, driven by the protocol fixture over the host's own API.
for name in saas core-lib; do api POST api/projects/trust "{\"path\":\"$tmp/$name\"}" >/dev/null; done

# One that reaches its gates having skipped the burst test; the review must lead with it.
rate=$(api POST api/changes "{\"cwd\":\"$tmp/saas\",\"title\":\"Rate-limit the login route\",
  \"agent\":\"$ECHO\",\"spec\":\"specs/042-login-rate-limit\",\"tasks\":[\"US1\"],
  \"prompt\":\"write-file src/auth/limit.ts\"}")
wt=$(printf '%s' "$rate" | grep -oE '"worktree": *"[^"]+"' | head -1 | sed 's/.*"worktree": *"//; s/"$//')
[ -n "$wt" ] && [ -d "$wt" ] || { echo "seed-host: the change has no worktree: $rate" >&2; exit 1; }
printf 'test("login", () => {});\nit.skip("rejects the sixth attempt in a minute", () => {});\n' > "$wt/tests/login.test.ts"
# One that stops to ask the person something.
api POST api/changes "{\"cwd\":\"$tmp/saas\",\"title\":\"Drop the v1 API\",
  \"agent\":\"$ECHO\",\"spec\":\"specs/043-drop-v1-api\",\"prompt\":\"question: which clients still call v1?\"}" >/dev/null
# One in flight on another project.
api POST api/changes "{\"cwd\":\"$tmp/core-lib\",\"title\":\"Retry only on 5xx\",
  \"agent\":\"$ECHO\",\"prompt\":\"slowish plan\"}" >/dev/null

# Let the host fold what the hook processes appended, and the fixture finish.
sleep 4

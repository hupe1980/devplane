#!/usr/bin/env bash
# Downloads the third-party specs and docs referenced by CONCEPT.md into specs/ (gitignored).
# Usage: scripts/fetch-specs.sh
set -u
cd "$(dirname "$0")/.."
mkdir -p specs/claude-code specs/claude-agent-sdk specs/codex specs/opencode specs/symphony specs/sdd
UA='vibeplane-specs-fetch'
fetch() { # url dest
  local code sz
  code=$(curl -sL -A "$UA" -o "$2" -w '%{http_code}' "$1"); sz=$(wc -c < "$2" | tr -d ' ')
  if [ "$code" = 200 ] && [ "$sz" -gt 500 ] && ! head -c 300 "$2" | grep -qi '<!doctype html\|<html'; then
    echo "OK   $2"
  else
    echo "FAIL $code $1"; rm -f "$2"
  fi
}
# Claude Code docs (Mintlify serves markdown at <page>.md)
for p in hooks hooks-guide headless cli-reference worktrees sessions agent-view statusline channels permissions \
         permission-modes settings-reference agent-teams cross-session-messaging sub-agents tools-reference mcp \
         remote-control desktop claude-directory env-vars checkpointing; do
  fetch "https://code.claude.com/docs/en/$p.md" "specs/claude-code/$p.md"
done
fetch https://code.claude.com/docs/llms.txt specs/claude-code/llms.txt
# Claude Agent SDK docs
for p in overview permissions user-input streaming-output structured-outputs sessions mcp hooks typescript python cost-tracking; do
  fetch "https://code.claude.com/docs/en/agent-sdk/$p.md" "specs/claude-agent-sdk/$p.md"
done
fetch https://raw.githubusercontent.com/Roasbeef/claude-agent-sdk-go/main/docs/cli-protocol.md specs/claude-agent-sdk/community-cli-wire-protocol.md
# OpenAI Symphony (orchestration spec, Apache-2.0)
fetch https://raw.githubusercontent.com/openai/symphony/main/SPEC.md specs/symphony/SPEC.md
fetch https://raw.githubusercontent.com/openai/symphony/main/README.md specs/symphony/README.md
# Spec-driven development frameworks
fetch https://raw.githubusercontent.com/Fission-AI/OpenSpec/main/README.md specs/sdd/openspec-README.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/README.md specs/sdd/spec-kit-README.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/spec-driven.md specs/sdd/spec-kit-spec-driven.md
# Codex app-server (JSON-RPC)
fetch https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server/README.md specs/codex/app-server-README.md
# Codex app-server protocol: generated JSON Schema + TypeScript types (listed via GitHub API)
mkdir -p specs/codex/schema
for sub in json typescript; do
  curl -s -A "$UA" "https://api.github.com/repos/openai/codex/contents/codex-rs/app-server-protocol/schema/$sub" \
    | python3 -c 'import sys,json; [print(x["download_url"]) for x in json.load(sys.stdin) if x["type"]=="file"]' \
    | while read -r u; do [ -n "$u" ] && fetch "$u" "specs/codex/schema/$(basename "$u")"; done
done
# OpenCode server (HTTP + SSE)
fetch https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/web/src/content/docs/server.mdx specs/opencode/server.mdx
fetch https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/web/src/content/docs/sdk.mdx specs/opencode/sdk.mdx
fetch https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/sdk/openapi.json specs/opencode/openapi.json
# claude-view (closest existing observer)
fetch https://raw.githubusercontent.com/tombelieber/claude-view/main/README.md specs/claude-view-README.md
date -u +'fetched: %Y-%m-%dT%H:%MZ' > specs/FETCHED.txt

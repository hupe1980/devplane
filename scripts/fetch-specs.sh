#!/usr/bin/env bash
# Downloads the third-party specs and docs referenced by CONCEPT.md into specs/ (gitignored).
# Usage: scripts/fetch-specs.sh
set -u
cd "$(dirname "$0")/.."
mkdir -p specs/claude-code specs/claude-agent-sdk specs/copilot specs/codex specs/opencode specs/symphony specs/sdd specs/mcp specs/jsonrpc specs/acp specs/standards
UA='vibeplane-specs-fetch'
# The test is "did we get the document or an error page", and size was a bad proxy for it:
# a `-gt 500` floor deleted 83 of the 130 generated Codex schema files, because a generated
# TypeScript type alias is legitimately four lines long. A guard that removes the thing it
# was checking is worse than no guard, and this one reported the deletion as FAIL 200 —
# a success code beside the word FAIL, which is what it looks like when the check is wrong
# rather than the fetch. The HTML sniff is the real test; size only has to be non-zero.
fetch() { # url dest
  local code sz
  code=$(curl -sL -A "$UA" -o "$2" -w '%{http_code}' "$1"); sz=$(wc -c < "$2" | tr -d ' ')
  if [ "$code" = 200 ] && [ "$sz" -gt 0 ] && ! head -c 300 "$2" | grep -qi '<!doctype html\|<html'; then
    echo "OK   $2"
  else
    echo "FAIL $code $1"; rm -f "$2"
  fi
}
# Claude Code docs (Mintlify serves markdown at <page>.md)
for p in hooks hooks-guide headless cli-reference worktrees sessions agent-view statusline channels permissions monitoring-usage \
         permission-modes settings-reference agent-teams cross-session-messaging sub-agents tools-reference mcp \
         remote-control desktop claude-directory env-vars checkpointing \
         workflows sandboxing deep-links goal auto-mode-config skills plugins plugin-evals commands \
         scheduled-tasks routines agents vs-code claude-code-on-the-web accessibility keybindings \
         ultrareview code-review advisor feature-availability context-window \
         channels-reference plugins-reference errors github-actions gitlab-ci-cd \
         sandbox-environments analytics artifacts; do
  fetch "https://code.claude.com/docs/en/$p.md" "specs/claude-code/$p.md"
done
fetch https://code.claude.com/docs/llms.txt specs/claude-code/llms.txt
# The vendor publishes a dated weekly digest of what changed. Re-verification reads these
# rather than diffing a 300-page index by hand: a surface that landed since the last pass
# is a row in one of them.
fetch https://code.claude.com/docs/en/whats-new/index.md specs/claude-code/whats-new.md
for w in 37 36 35 34 33 32 31 30 29 28 27 26; do
  fetch "https://code.claude.com/docs/en/whats-new/2026-w$w.md" "specs/claude-code/whats-new-2026-w$w.md"
done
# A week with no digest is normal and is not an absence of change: the digest stopped at week
# 34 while the product reached 2.1.270, so thirty releases — including a sixth permission-rule
# widening — exist only in the CHANGELOG. The digest tells you what the vendor thought was
# notable; the changelog is the enumerated table.
fetch https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md specs/claude-code/CHANGELOG.md
# Claude Agent SDK docs
for p in overview permissions user-input streaming-output structured-outputs sessions mcp hooks typescript python cost-tracking \
         session-storage observability todo-tracking subagents; do
  fetch "https://code.claude.com/docs/en/agent-sdk/$p.md" "specs/claude-agent-sdk/$p.md"
done
fetch https://raw.githubusercontent.com/Roasbeef/claude-agent-sdk-go/main/docs/cli-protocol.md specs/claude-agent-sdk/community-cli-wire-protocol.md
# GitHub Copilot: the second provider that documents all three channels.
# GitHub publishes its docs as markdown in github/docs, so these are the source files
# rather than a rendered page.
CPD=https://raw.githubusercontent.com/github/docs/main/content/copilot
fetch "$CPD/reference/hooks-reference.md"                              specs/copilot/hooks-reference.md
fetch "$CPD/reference/copilot-cli-reference/acp-server.md"             specs/copilot/acp-server.md
fetch "$CPD/reference/copilot-cli-reference/cli-command-reference.md"  specs/copilot/cli-command-reference.md
fetch "$CPD/reference/copilot-cli-reference/cli-config-dir-reference.md" specs/copilot/cli-config-dir-reference.md
fetch "$CPD/how-tos/copilot-cli/use-copilot-cli/allowing-tools.md"     specs/copilot/allowing-tools.md
fetch "$CPD/how-tos/copilot-sdk/observability/opentelemetry.md"        specs/copilot/sdk-opentelemetry.md
# OpenAI Symphony (orchestration spec, Apache-2.0)
fetch https://raw.githubusercontent.com/openai/symphony/main/SPEC.md specs/symphony/SPEC.md
fetch https://raw.githubusercontent.com/openai/symphony/main/README.md specs/symphony/README.md
# Spec-driven development frameworks
fetch https://raw.githubusercontent.com/Fission-AI/OpenSpec/main/README.md specs/sdd/openspec-README.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/README.md specs/sdd/spec-kit-README.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/spec-driven.md specs/sdd/spec-kit-spec-driven.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/commands/analyze.md specs/sdd/spec-kit-analyze.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/commands/converge.md specs/sdd/spec-kit-converge.md
# The two templates that carry the shape a work item names: what a specification
# document holds, and how its task list is written. `--spec` reads the second.
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/spec-template.md specs/sdd/spec-kit-spec-template.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/tasks-template.md specs/sdd/spec-kit-tasks-template.md
fetch https://agents.md/ specs/standards/agents-md.md
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
# Model Context Protocol (the gate server speaks it) + JSON-RPC 2.0 (daemon API, Codex app-server)
MCP_REV=2026-07-28
for p in basic/index server/tools basic/transports client/elicitation; do
  fetch "https://modelcontextprotocol.io/specification/$MCP_REV/$p.md" "specs/mcp/$(echo "$p" | tr '/' '-' | sed 's/-index$//').md"
done
fetch "https://modelcontextprotocol.io/specification/$MCP_REV.md" "specs/mcp/spec-$MCP_REV.md"
fetch https://modelcontextprotocol.io/llms.txt specs/mcp/llms.txt
fetch "https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/$MCP_REV/schema.json" "specs/mcp/schema-$MCP_REV.json"
rm -f specs/mcp/spec-2025-06-18.md specs/mcp/schema-2025-06-18.json
fetch https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/README.md specs/mcp/rmcp-README.md
fetch https://www.jsonrpc.org/specification specs/jsonrpc/jsonrpc-2.0.html
# Agent Client Protocol: protocol pages (v1/v2), Rust SDK page, registry docs + JSON, READMEs
for p in v2/overview v2/initialization v2/session-setup v2/session-list v2/prompt-lifecycle v2/tool-calls v2/agent-plan v2/elicitation v2/cancellation v2/extensibility v1/overview v1/file-system v1/terminals v1/session-modes; do
  fetch "https://agentclientprotocol.com/protocol/$p.md" "specs/acp/$(echo "$p" | tr '/' '-').md"
done
fetch https://agentclientprotocol.com/libraries/rust.md specs/acp/libraries-rust.md
fetch https://agentclientprotocol.com/get-started/registry.md specs/acp/registry.md
fetch https://agentclientprotocol.com/llms.txt specs/acp/llms.txt
fetch https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json specs/acp/registry.json
fetch https://raw.githubusercontent.com/agentclientprotocol/agent-client-protocol/main/README.md specs/acp/agent-client-protocol-README.md
fetch https://raw.githubusercontent.com/agentclientprotocol/claude-agent-acp/main/README.md specs/acp/claude-agent-acp-README.md
fetch https://raw.githubusercontent.com/agentclientprotocol/registry/main/README.md specs/acp/registry-README.md
# claude-view (closest existing observer)
fetch https://raw.githubusercontent.com/tombelieber/claude-view/main/README.md specs/claude-view-README.md
date -u +'fetched: %Y-%m-%dT%H:%MZ' > specs/FETCHED.txt

#!/usr/bin/env bash
# Downloads the third-party specs and docs the notes cite into concepts/reference/ (gitignored).
# Usage: scripts/fetch-reference.sh
set -u
cd "$(dirname "$0")/.."
mkdir -p concepts/reference/claude-code concepts/reference/claude-agent-sdk concepts/reference/copilot concepts/reference/codex concepts/reference/opencode concepts/reference/symphony concepts/reference/sdd concepts/reference/mcp concepts/reference/jsonrpc concepts/reference/acp concepts/reference/standards
UA='devplane-specs-fetch'
# An HTML body means an error page, not the document; size need only be non-zero.
# A failed fetch keeps the previous copy and reports it stale, so a broken fetch
# cannot erase a fact that is still true.
fetch() { # url dest
  local code sz
  local tmp="$2.fetching"
  code=$(curl -sL -A "$UA" -o "$tmp" -w '%{http_code}' "$1"); sz=$(wc -c < "$tmp" | tr -d ' ')
  if [ "$code" = 200 ] && [ "$sz" -gt 0 ] && ! head -c 300 "$tmp" | grep -qi '<!doctype html\|<html'; then
    mv "$tmp" "$2"; echo "OK   $2"
  elif [ -s "$2" ]; then
    rm -f "$tmp"; echo "STALE $code $1 (keeping the copy already on disk)"
  else
    rm -f "$tmp"; echo "FAIL $code $1"
  fi
}

# A page that serves only HTML, rendered to text. Used for `agents.md` only, which
# publishes no markdown.
fetch_html_as_text() { # url dest
  local code tmp="$2.fetching"
  code=$(curl -sL -A "$UA" -o "$tmp" -w '%{http_code}' "$1")
  if [ "$code" = 200 ] && [ -s "$tmp" ]; then
    # Script bodies are kept: this Next.js page's prose lives in `__NEXT_DATA__`, and a
    # greedy script strip on a minified page deletes everything.
    sed -e 's/<[^>]*>/\n/g' "$tmp" \
      | sed -e 's/&amp;/\&/g; s/&lt;/</g; s/&gt;/>/g; s/&quot;/"/g; s/&#x27;/'"'"'/g; s/&#39;/'"'"'/g; s/&nbsp;/ /g; s/\\u0026/\&/g' \
      | tr -s ' \t' ' ' | grep -v '^ *$' > "$2.text"
    if [ "$(wc -c < "$2.text" | tr -d ' ')" -gt 1000 ]; then
      mv "$2.text" "$2"; rm -f "$tmp"; echo "OK   $2 (html -> text)"; return
    fi
    rm -f "$2.text"
  fi
  rm -f "$tmp"
  if [ -s "$2" ]; then echo "STALE $code $1 (keeping the copy already on disk)"; else echo "FAIL $code $1"; fi
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
  fetch "https://code.claude.com/docs/en/$p.md" "concepts/reference/claude-code/$p.md"
done
fetch https://code.claude.com/docs/llms.txt concepts/reference/claude-code/llms.txt
# The vendor's dated weekly digests of what changed.
fetch https://code.claude.com/docs/en/whats-new/index.md concepts/reference/claude-code/whats-new.md
for w in 37 36 35 34 33 32 31 30 29 28 27 26; do
  fetch "https://code.claude.com/docs/en/whats-new/2026-w$w.md" "concepts/reference/claude-code/whats-new-2026-w$w.md"
done
# The digests are selective; the CHANGELOG is the complete list.
fetch https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md concepts/reference/claude-code/CHANGELOG.md
# Claude Agent SDK docs
for p in overview permissions user-input streaming-output structured-outputs sessions mcp hooks typescript python cost-tracking \
         session-storage observability todo-tracking subagents; do
  fetch "https://code.claude.com/docs/en/agent-sdk/$p.md" "concepts/reference/claude-agent-sdk/$p.md"
done
fetch https://raw.githubusercontent.com/Roasbeef/claude-agent-sdk-go/main/docs/cli-protocol.md concepts/reference/claude-agent-sdk/community-cli-wire-protocol.md
# GitHub Copilot: the second provider that documents all three channels.
# Fetched as the markdown sources in github/docs.
CPD=https://raw.githubusercontent.com/github/docs/main/content/copilot
fetch "$CPD/reference/hooks-reference.md"                              concepts/reference/copilot/hooks-reference.md
fetch "$CPD/reference/copilot-cli-reference/acp-server.md"             concepts/reference/copilot/acp-server.md
fetch "$CPD/reference/copilot-cli-reference/cli-command-reference.md"  concepts/reference/copilot/cli-command-reference.md
fetch "$CPD/reference/copilot-cli-reference/cli-config-dir-reference.md" concepts/reference/copilot/cli-config-dir-reference.md
fetch "$CPD/how-tos/copilot-cli/use-copilot-cli/allowing-tools.md"     concepts/reference/copilot/allowing-tools.md
fetch "$CPD/how-tos/copilot-sdk/observability/opentelemetry.md"        concepts/reference/copilot/sdk-opentelemetry.md
# OpenAI Symphony (orchestration spec, Apache-2.0)
fetch https://raw.githubusercontent.com/openai/symphony/main/SPEC.md concepts/reference/symphony/SPEC.md
fetch https://raw.githubusercontent.com/openai/symphony/main/README.md concepts/reference/symphony/README.md
# Spec-driven development frameworks
fetch https://raw.githubusercontent.com/Fission-AI/OpenSpec/main/README.md concepts/reference/sdd/openspec-README.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/README.md concepts/reference/sdd/spec-kit-README.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/spec-driven.md concepts/reference/sdd/spec-kit-spec-driven.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/commands/analyze.md concepts/reference/sdd/spec-kit-analyze.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/commands/converge.md concepts/reference/sdd/spec-kit-converge.md
# What a specification holds and how its task list is written; `--spec` reads the latter.
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/spec-template.md concepts/reference/sdd/spec-kit-spec-template.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/tasks-template.md concepts/reference/sdd/spec-kit-tasks-template.md
# HTML-only; see `fetch_html`.
fetch_html_as_text https://agents.md/ concepts/reference/standards/agents-md.md
# Codex app-server (JSON-RPC)
fetch https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server/README.md concepts/reference/codex/app-server-README.md
# Codex app-server protocol: generated JSON Schema + TypeScript types (listed via GitHub API)
mkdir -p concepts/reference/codex/schema
for sub in json typescript; do
  curl -s -A "$UA" "https://api.github.com/repos/openai/codex/contents/codex-rs/app-server-protocol/schema/$sub" \
    | python3 -c 'import sys,json; [print(x["download_url"]) for x in json.load(sys.stdin) if x["type"]=="file"]' \
    | while read -r u; do [ -n "$u" ] && fetch "$u" "concepts/reference/codex/schema/$(basename "$u")"; done
done
# OpenCode server (HTTP + SSE)
fetch https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/web/src/content/docs/server.mdx concepts/reference/opencode/server.mdx
fetch https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/web/src/content/docs/sdk.mdx concepts/reference/opencode/sdk.mdx
fetch https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/sdk/openapi.json concepts/reference/opencode/openapi.json
# Model Context Protocol (the gate server speaks it) + JSON-RPC 2.0 (Codex app-server)
MCP_REV=2026-07-28
for p in basic/index server/tools basic/transports client/elicitation; do
  fetch "https://modelcontextprotocol.io/specification/$MCP_REV/$p.md" "concepts/reference/mcp/$(echo "$p" | tr '/' '-' | sed 's/-index$//').md"
done
fetch "https://modelcontextprotocol.io/specification/$MCP_REV.md" "concepts/reference/mcp/spec-$MCP_REV.md"
fetch https://modelcontextprotocol.io/llms.txt concepts/reference/mcp/llms.txt
fetch "https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/$MCP_REV/schema.json" "concepts/reference/mcp/schema-$MCP_REV.json"
rm -f concepts/reference/mcp/spec-2025-06-18.md concepts/reference/mcp/schema-2025-06-18.json
fetch https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/README.md concepts/reference/mcp/rmcp-README.md
fetch https://www.jsonrpc.org/specification concepts/reference/jsonrpc/jsonrpc-2.0.html
# Agent Client Protocol: protocol pages (v1/v2), Rust SDK page, registry docs + JSON, READMEs
for p in v2/overview v2/initialization v2/session-setup v2/session-list v2/prompt-lifecycle v2/tool-calls v2/agent-plan v2/elicitation v2/cancellation v2/extensibility v1/overview v1/file-system v1/terminals v1/session-modes; do
  fetch "https://agentclientprotocol.com/protocol/$p.md" "concepts/reference/acp/$(echo "$p" | tr '/' '-').md"
done
fetch https://agentclientprotocol.com/libraries/rust.md concepts/reference/acp/libraries-rust.md
fetch https://agentclientprotocol.com/get-started/registry.md concepts/reference/acp/registry.md
fetch https://agentclientprotocol.com/llms.txt concepts/reference/acp/llms.txt
fetch https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json concepts/reference/acp/registry.json
fetch https://raw.githubusercontent.com/agentclientprotocol/agent-client-protocol/main/README.md concepts/reference/acp/agent-client-protocol-README.md
fetch https://raw.githubusercontent.com/agentclientprotocol/claude-agent-acp/main/README.md concepts/reference/acp/claude-agent-acp-README.md
fetch https://raw.githubusercontent.com/agentclientprotocol/registry/main/README.md concepts/reference/acp/registry-README.md
# claude-view (closest existing observer)
fetch https://raw.githubusercontent.com/tombelieber/claude-view/main/README.md concepts/reference/claude-view-README.md
# Agent Skills: the portable core, and the authority on which six fields are portable.
mkdir -p concepts/reference/standards
fetch https://agentskills.io/specification.md concepts/reference/standards/agent-skills-spec.md
fetch https://agentskills.io/llms.txt concepts/reference/standards/agent-skills-llms.txt
# ── Papers ───────────────────────────────────────────────────────────────────
# arXiv HTML rather than PDF so claims are greppable; tags are stripped. A paper
# with no HTML rendering is fetched as its abstract, so body-only checks will MISS.
arxiv() { # id dest
  local dest="concepts/reference/papers/$2.txt" code
  mkdir -p concepts/reference/papers
  code=$(curl -sL -A "$UA" -o /tmp/dp-arxiv.$$ -w '%{http_code}' "https://arxiv.org/html/$1v1")
  if [ "$code" != 200 ]; then
    code=$(curl -sL -A "$UA" -o /tmp/dp-arxiv.$$ -w '%{http_code}' "https://arxiv.org/abs/$1")
    [ "$code" = 200 ] && echo "NOTE $1 has no HTML rendering; abstract only"
  fi
  if [ "$code" = 200 ]; then
    python3 -c '
import sys, re, html
t = open(sys.argv[1], encoding="utf-8", errors="replace").read()
t = re.sub(r"<(script|style).*?</\1>", " ", t, flags=re.S | re.I)
t = re.sub(r"<[^>]+>", " ", t)
t = re.sub(r"[ \t]+", " ", html.unescape(t))
open(sys.argv[2], "w", encoding="utf-8").write(t)
' /tmp/dp-arxiv.$$ "$dest" && echo "OK   $dest"
  else
    echo "FAIL $code arxiv:$1"
  fi
  rm -f /tmp/dp-arxiv.$$
}
arxiv 2607.28317 oversight-vacuity
arxiv 2606.08919 oversight-capacity
arxiv 2606.05647 sabotage-detection
arxiv 2607.25152 self-evaluation-bias
arxiv 2604.05485 auditability-dimensions

date -u +'fetched: %Y-%m-%dT%H:%MZ' > concepts/reference/FETCHED.txt

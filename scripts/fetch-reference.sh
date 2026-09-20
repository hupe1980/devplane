#!/usr/bin/env bash
# Downloads the third-party specs and docs referenced by CONCEPT.md into concepts/reference/ (gitignored).
# Usage: scripts/fetch-reference.sh
set -u
cd "$(dirname "$0")/.."
mkdir -p concepts/reference/claude-code concepts/reference/claude-agent-sdk concepts/reference/copilot concepts/reference/codex concepts/reference/opencode concepts/reference/symphony concepts/reference/sdd concepts/reference/mcp concepts/reference/jsonrpc concepts/reference/acp concepts/reference/standards
UA='devplane-specs-fetch'
# The test is "did we get the document or an error page", and size was a bad proxy for it:
# a `-gt 500` floor deleted 83 of the 130 generated Codex schema files, because a generated
# TypeScript type alias is legitimately four lines long. A guard that removes the thing it
# was checking is worse than no guard, and this one reported the deletion as FAIL 200 —
# a success code beside the word FAIL, which is what it looks like when the check is wrong
# rather than the fetch. The HTML sniff is the real test; size only has to be non-zero.
#
# And a guard may refuse, but it may not destroy. This function used to `rm -f`
# the destination on a failed fetch, and on 2026-09-19 that deleted a page that
# had been correct for five passes: `agents.md` moved to a Next.js site with no
# markdown endpoint, the HTML sniff refused it — correctly — and then removed
# the good copy underneath it. Three claims went from pinned to MISS, and every
# one of the three is still true on the live page. **The fetch broke, not the
# fact**, and a destructive guard makes those two indistinguishable. A failed
# fetch now leaves the previous copy where it is and says it is stale; only a
# page nobody has ever fetched is absent, which is the one case the claim ledger
# should fail on.
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

# A page that serves only HTML, rendered to text. Used for exactly one source and
# deliberately not made general: `agents.md` is the governance evidence for the one
# standard in these notes that has any, it publishes no markdown, and a claim about
# it is worth a few lines of `sed` rather than a footnote saying it could not be
# checked. Everything else in this file is fetched as markdown or not at all.
fetch_html_as_text() { # url dest
  local code tmp="$2.fetching"
  code=$(curl -sL -A "$UA" -o "$tmp" -w '%{http_code}' "$1")
  if [ "$code" = 200 ] && [ -s "$tmp" ]; then
    # Tags become newlines and entities become characters; script and style
    # bodies are deliberately *kept*, because this page is a Next.js build whose
    # readable prose lives in a `__NEXT_DATA__` JSON blob rather than in the
    # markup. Stripping scripts the tidy way produced an 11-byte file — and, on
    # a minified single-line document, a greedy `s/<script.*<\/script>//`
    # deletes everything between the first script and the last one, which is the
    # whole page. Kept as a warning: the tidier transformation was the one that
    # silently destroyed the content.
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
# The vendor publishes a dated weekly digest of what changed. Re-verification reads these
# rather than diffing a 300-page index by hand: a surface that landed since the last pass
# is a row in one of them.
fetch https://code.claude.com/docs/en/whats-new/index.md concepts/reference/claude-code/whats-new.md
for w in 37 36 35 34 33 32 31 30 29 28 27 26; do
  fetch "https://code.claude.com/docs/en/whats-new/2026-w$w.md" "concepts/reference/claude-code/whats-new-2026-w$w.md"
done
# A week with no digest is normal and is not an absence of change: the digest stopped at week
# 34 while the product reached 2.1.270, so thirty releases — including a sixth permission-rule
# widening — exist only in the CHANGELOG. The digest tells you what the vendor thought was
# notable; the changelog is the enumerated table.
fetch https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md concepts/reference/claude-code/CHANGELOG.md
# Claude Agent SDK docs
for p in overview permissions user-input streaming-output structured-outputs sessions mcp hooks typescript python cost-tracking \
         session-storage observability todo-tracking subagents; do
  fetch "https://code.claude.com/docs/en/agent-sdk/$p.md" "concepts/reference/claude-agent-sdk/$p.md"
done
fetch https://raw.githubusercontent.com/Roasbeef/claude-agent-sdk-go/main/docs/cli-protocol.md concepts/reference/claude-agent-sdk/community-cli-wire-protocol.md
# GitHub Copilot: the second provider that documents all three channels.
# GitHub publishes its docs as markdown in github/docs, so these are the source files
# rather than a rendered page.
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
# The two templates that carry the shape a work item names: what a specification
# document holds, and how its task list is written. `--spec` reads the second.
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/spec-template.md concepts/reference/sdd/spec-kit-spec-template.md
fetch https://raw.githubusercontent.com/github/spec-kit/main/templates/tasks-template.md concepts/reference/sdd/spec-kit-tasks-template.md
# HTML-only since 2026-09: the format with the governance is the one whose own page
# cannot be fetched as markdown.
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
# Model Context Protocol (the gate server speaks it) + JSON-RPC 2.0 (daemon API, Codex app-server)
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
# Agent Skills: the portable core the library carries, and the only authority
# for which six fields survive leaving a vendor (#library, D272).
mkdir -p concepts/reference/standards
fetch https://agentskills.io/specification.md concepts/reference/standards/agent-skills-spec.md
fetch https://agentskills.io/llms.txt concepts/reference/standards/agent-skills-llms.txt
# ── Papers ───────────────────────────────────────────────────────────────────
#
# **Twenty-three papers were cited in these notes and nought were checkable.**
# `verify-claims.sh` has tested every vendor claim since it was written and had
# no arXiv entry at all, in a corpus whose own standing rule is that every
# number names its test. Reading one of them in full on 2026-09-19 found a
# correlation quoted in the opposite direction and a feature designed around a
# criterion the paper does not contain (D269).
#
# HTML rather than the PDF: arXiv renders most recent submissions, the text is
# greppable, and a claim check against a PDF is a claim check against nothing.
# Tags are stripped here so the checks downstream match prose rather than
# markup. A paper with no HTML rendering is fetched as its abstract page, and
# a check that needs the body will simply miss — which is the correct outcome
# and is why this does not fall back silently to something smaller.
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

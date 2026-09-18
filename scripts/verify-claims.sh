#!/usr/bin/env bash
# The claim ledger of concepts/QUALITY.md §2: every load-bearing integration claim in the
# concept notes is pinned to a file in concepts/reference/ and this script greps for it. Run
# scripts/fetch-reference.sh first. Exit 1 if any claim is missing from its source.
set -u
cd "$(dirname "$0")/../concepts/reference" || { echo "no concepts/reference/ (run scripts/fetch-reference.sh)"; exit 1; }
fail=0; n=0
chk() { n=$((n+1)); if grep -q -i -E "$3" $2 2>/dev/null; then printf 'OK   %s\n' "$1"; else printf 'MISS %s  (%s ~ /%s/)\n' "$1" "$2" "$3"; fail=1; fi; }
# ── GitHub Copilot: the second provider that documents all three channels ─────
chk "copilot: hooks accept an http handler"           copilot/hooks-reference.md '"type": "http"'
chk "copilot: a permission hook must use https"       copilot/hooks-reference.md 'must use .https://. because the response can grant tool permissions'
chk "copilot: an http preToolUse hook fails open"     copilot/hooks-reference.md 'HTTP .preToolUse. hooks are \*\*fail-open\*\*'
chk "copilot: a command preToolUse hook fails closed" copilot/hooks-reference.md 'Command .preToolUse. hooks are \*\*fail-closed\*\*'
chk "copilot: a command hook timeout fails open"      copilot/hooks-reference.md '[Tt]imeouts are always fail-open'
chk "copilot: preToolUse decides allow/deny/ask"      copilot/hooks-reference.md 'permissionDecision'
chk "copilot: permissionRequest precedes the rules"   copilot/hooks-reference.md 'before the permission service runs'
chk "copilot: hooks also run in the cloud agent"      copilot/hooks-reference.md 'Cloud agent'
chk "copilot: the default hook timeout is 30 s"       copilot/hooks-reference.md 'Timeout in seconds. Default: .30.'
chk "copilot: an ACP server on stdio or a port"       copilot/acp-server.md 'copilot --acp'
chk "copilot: ACP support is a public preview"        copilot/acp-server.md 'release-phases.public_preview'
chk "copilot: OTel is off until one var is set"       copilot/cli-command-reference.md 'COPILOT_OTEL_ENABLED'
chk "copilot: an endpoint alone enables OTel"         copilot/cli-command-reference.md 'OTEL_EXPORTER_OTLP_ENDPOINT. is set'
chk "copilot: OTLP http/json is the default"          copilot/cli-command-reference.md 'OTEL_EXPORTER_OTLP_PROTOCOL. \| .http/json'
chk "copilot: it exports traces and metrics"          copilot/cli-command-reference.md 'export traces and metrics'
chk "copilot: the schema is the GenAI conventions"    copilot/cli-command-reference.md 'GenAI Semantic Conventions'
chk "copilot: prompt content is off by default"       copilot/cli-command-reference.md 'OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT. \| .false'
chk "copilot: the service name names the vendor"      copilot/cli-command-reference.md 'github-copilot'
chk "copilot: tool rules are kind(specifier)"         copilot/allowing-tools.md "shell\(git:\*\)"
chk "copilot: deny beats allow"                       copilot/allowing-tools.md 'Deny rules always take precedence'
# ── The parity rows the CHANGELOG carries and the weekly digest does not ────────
chk "cc: a Bash tee target is checked as a write"     claude-code/CHANGELOG.md 'the file a Bash .tee. command writes'
chk "cc: file-command coverage is an open list"       claude-code/permissions.md 'such as .cat., .head., .tail., and .sed.'
chk "cc: a deny rule reaches a symlink's target"      claude-code/permissions.md 'apply when either the symlink path or its target matches'
chk "cc: an allow rule needs both to match"           claude-code/permissions.md 'apply only when both the symlink path and its target match'
chk "cc: a rule through a symlinked dir applies too"  claude-code/permissions.md 'also applies at the directory.s real location'
chk "cc: a negation rule is scoped to its source"     claude-code/CHANGELOG.md 'permission rule starting with .!. applying beyond the settings source'
chk "cc: Cd rules are directory-anchored, not gitignore" claude-code/permissions.md 'anchored to the whole directory path rather than gitignore-style'
chk "cc: an unparsed command always prompts"          claude-code/permissions.md 'Commands the analysis can.t parse'
chk "cc: read-only git forms need no prompt"          claude-code/permissions.md 'read-only forms of .git.'
# ── The surfaces the third index pass found ────────────────────────────────
chk "ultrareview: findings are independently verified"  claude-code/ultrareview.md 'independently reproduced and verified'
chk "ultrareview: runs in a remote cloud sandbox"       claude-code/ultrareview.md 'remote sandbox|cloud sandbox'
chk "ultrareview: a non-interactive subcommand exists"  claude-code/ultrareview.md 'claude ultrareview'
chk "ultrareview: --json prints the bugs payload"       claude-code/ultrareview.md 'bugs\.json'
chk "ultrareview: unavailable on Bedrock/Vertex/Foundry" claude-code/ultrareview.md 'not available when using Claude Code with Amazon Bedrock'
chk "code-review: a verification step filters findings" claude-code/code-review.md 'verification step checks candidates against actual code behavior'
chk "code-review: the check run never blocks a merge"   claude-code/code-review.md 'neutral conclusion so it never blocks merging'
chk "code-review: gating is handed to your own CI"      claude-code/code-review.md 'want to gate merges on Code Review findings'
chk "code-review: the severity payload is machine-readable" claude-code/code-review.md 'bughunter-severity'
chk "code-review: REVIEW.md is the project's own rules" claude-code/code-review.md 'REVIEW\.md'
chk "advisor: consulted before declaring a task done"   claude-code/advisor.md 'before declaring a task complete|before declaring a task done'
chk "advisor: the model decides when to call it"        claude-code/advisor.md 'Claude decides when to call it'
chk "availability: workflows run on every provider"     claude-code/feature-availability.md 'Workflows'
chk "availability: Remote Control needs a subscription" claude-code/feature-availability.md 'Remote Control'
chk "availability: Code Review is Team/Enterprise only" claude-code/feature-availability.md 'Code Review'
chk "availability: auto mode is partial off-Anthropic"  claude-code/feature-availability.md 'auto mode supports only'
chk "availability: those providers start in Manual"     claude-code/feature-availability.md 'starting permission mode on these providers is Manual'
chk "gitlab: findings post through the glab CLI"        claude-code/code-review.md 'glab'
chk "whats-new: the vendor publishes a dated weekly digest" claude-code/whats-new.md 'Week [0-9]+'
chk "hooks: http type"                              claude-code/hooks.md '"type": "http"'
chk "hooks: async flag"                             claude-code/hooks.md '"async": true'
chk "hooks: allowedHttpHookUrls"                    claude-code/hooks.md 'allowedHttpHookUrls'
chk "hooks: httpHookAllowedEnvVars"                 claude-code/hooks.md 'httpHookAllowedEnvVars'
chk "hooks: Notification permission_prompt"         claude-code/hooks.md 'permission_prompt'
chk "hooks: elicitation_dialog matcher"             claude-code/hooks.md 'elicitation_dialog'
chk "hooks: agent_needs_input only in agent view"   claude-code/hooks.md 'agent_needs_input.*agent view|while \[agent view\]'
chk "hooks: PermissionRequest decision object"      claude-code/hooks.md '"decision"'
chk "hooks: transcript_path in common input"        claude-code/hooks.md 'transcript_path'
chk "hooks: PostCompact is after compaction"        claude-code/hooks.md 'PostCompact.*\| *After context compaction'
chk "hooks: PostModelSwitch after a model change"   claude-code/hooks.md 'PostModelSwitch.*\| *After the session.s model changes'
chk "hooks: Elicitation when MCP asks the user"     claude-code/hooks.md 'Elicitation.*\| *When an MCP server requests user input'
chk "hooks: PermissionDenied on an auto denial"     claude-code/hooks.md 'PermissionDenied.*\| *When auto mode denies'
# The permission-rule syntax. Six of these were claimed as implemented for months and were
# not; each row is one way for a deny rule to match nothing without saying so.
chk "policy: :* is a trailing wildcard"             claude-code/permissions.md 'The .:\*. suffix is an equivalent way to write a trailing wildcard'
chk "policy: :* only at the end"                    claude-code/permissions.md 'only recognized at the end of a pattern'
chk "policy: a trailing * covers the bare command"  claude-code/permissions.md 'also matches the bare command'
chk "policy: Read/Edit use gitignore syntax"        claude-code/permissions.md 'use \[gitignore\].* pattern syntax'
chk "policy: // is the filesystem root"             claude-code/permissions.md 'Absolute path from filesystem root'
chk "policy: / anchors at the settings source"      claude-code/permissions.md 'Path relative to the settings source'
chk "policy: a bare filename matches any depth"     claude-code/permissions.md 'Bare filenames follow gitignore semantics and match at any depth'
chk "policy: single-segment dir floats on deny"     claude-code/permissions.md 'matches a directory named .secrets. at any depth'
chk "policy: Edit covers every editing tool"        claude-code/permissions.md 'Edit. rules apply to all built-in tools that edit files'

# The files a shell command names. Each of these was a prohibition that
# read as protection and provided none until the matcher reached them.
chk "policy: an output redirect is checked as a write"  claude-code/permissions.md 'as if Claude wrote or read that file directly'
chk "policy: output redirects use Edit rules"           claude-code/permissions.md 'the check covers your .Edit. allow and deny rules'
chk "policy: input redirects use Read rules"            claude-code/permissions.md 'the check covers your .Read. allow and deny rules'
chk "policy: a command rule is not a target rule"       claude-code/permissions.md 'allows the command, not the target'
chk "policy: deny rules reach Bash file commands"       claude-code/permissions.md 'file commands Claude Code recognizes in Bash'
chk "policy: a Read deny does not reach NotebookEdit"   claude-code/permissions.md 'NotebookEdit isn.t covered'
chk "policy: no file behind these targets"              claude-code/permissions.md 'aren.t checked'

# The second gate. `PermissionRequest` does not fire in auto mode, so a
# prohibition answered only there does not run in the mode people pick when
# they are not watching.
chk "hooks: PreToolUse runs before every tool call"     claude-code/hooks.md 'PreToolUse hooks run before every tool call'
chk "hooks: PermissionRequest only when about to ask"   claude-code/hooks.md 'run only when Claude Code is about to ask you for permission'
chk "hooks: a hook ask forces a prompt in auto mode"    claude-code/hooks.md "also forces a permission prompt in \\[auto mode\\]"
chk "hooks: the classifier cannot approve past an ask"  claude-code/hooks.md "can.t approve the call silently"
chk "hooks: PreToolUse decides in hookSpecificOutput"   claude-code/hooks.md 'permissionDecision'
chk "modes: auto mode reviews with a classifier"        claude-code/permission-modes.md 'classifier model reviews actions before they run'
chk "policy: a Read deny blocks writes too"         claude-code/permissions.md 'Read. deny rule also blocks the'
chk "policy: path rules only on Read and Edit"      claude-code/permissions.md 'checks file permissions against .Edit\(path\). and .Read\(path\). rules only'
chk "policy: Grep/Glob primary field is path"       claude-code/permissions.md '.path. for Grep and Glob'
chk "policy: tool-name globs are deny-side"         claude-code/permissions.md 'Deny and ask rules also accept glob patterns in the tool-name position'
chk "policy: an allow glob must be anchored"        claude-code/permissions.md 'Allow rules accept tool-name globs only after a literal'
chk "policy: mcp__server covers a server"           claude-code/permissions.md 'matches any tool provided by the .puppeteer. server'
chk "policy: mcp__ rules with brackets are skipped" claude-code/permissions.md 'skips any .mcp__. rule that has parentheses'
chk "policy: parameter rules are deny-side"         claude-code/permissions.md 'Deny and ask rules can match a top-level input parameter'
chk "policy: no rule on a primary content field"    claude-code/permissions.md 'You can.t match a tool.s primary content field this way'
chk "changelog: PermissionRequest fires in --print" claude-code/CHANGELOG.md 'PermissionRequest hooks not firing in .--print. mode'
chk "agent-view: --json"                            claude-code/agent-view.md 'agents --json'
chk "agent-view: waitingFor"                        claude-code/agent-view.md 'waitingFor'
chk "agent-view: single cwd / --cwd"                claude-code/agent-view.md '\-\-cwd'
# D79/D80: the three facts the policy matcher is built on. Each one was wrong in
# the implementation until it was read off this file, so each is pinned to it.
chk "permissions: deny then ask then allow"        claude-code/permissions.md 'evaluated in order: deny, then ask, then allow'
chk "permissions: ask outranks a narrower allow"   claude-code/permissions.md 'ask rule prompts even when a more specific allow rule'
chk "permissions: rules split on shell operators"  claude-code/permissions.md 'must match each subcommand independently'
chk "permissions: deny reaches nested commands"    claude-code/permissions.md 'nested inside a subshell, a command substitution, or a control-flow body'
chk "permissions: unfinished command approves none" claude-code/permissions.md "doesn't split it into subcommands for allow-rule matching"
chk "permissions: the wrapper list"                claude-code/permissions.md 'timeout`, `time`, `nice`, `nohup`, and `stdbuf'
chk "permissions: deny looks past any assignment"  claude-code/permissions.md 'deny or ask rule matches past any leading assignment'
chk "permissions: unknown tool name warns"         claude-code/permissions.md 'matches no known tool produces a startup warning'
chk "permissions: Stop Task is TaskStop"           claude-code/permissions.md 'canonical name `TaskStop`'

# `core::policy::KNOWN_TOOLS` is a snapshot of somebody else's tool reference,
# used to catch a typo in a prohibition. It only ever warns, but a list that has
# rotted warns about tools that exist — so the reference has to still contain
# every name in it.
# Paths are relative to concepts/reference/, because that is where this script runs — so the
# tree is two levels up (`../../src`) and the notes are one (`../STATE.md`). Both were one
# level closer until the corpus moved under concepts/ on 2026-09-18, and the KNOWN_TOOLS
# check reported SKIP rather than failing when its source path stopped resolving, which is
# the failure this file is otherwise built to prevent: a check that stops checking, quietly.
if [ -f claude-code/tools-reference.md ] && [ -f ../../src/core/policy.rs ]; then
  missing=""
  for t in $(sed -n '/^const KNOWN_TOOLS/,/^];/p' ../../src/core/policy.rs \
             | grep -oE '"[A-Za-z]+"' | tr -d '"'); do
    case "$t" in
      MultiEdit) continue ;;   # Claude Code's legacy name, kept for pasted files
    esac
    grep -q "\`$t\`" claude-code/tools-reference.md || missing="$missing $t"
  done
  n=$((n+1))
  if [ -n "$missing" ]; then
    printf 'MISS %s  (not in the tools reference:%s)\n' "policy: KNOWN_TOOLS matches the reference" "$missing"
    fail=1
  else
    printf 'OK   %s\n' "policy: KNOWN_TOOLS matches the reference"
  fi
else
  printf 'SKIP policy: KNOWN_TOOLS check (specs or source not found)\n'
fi

chk "permissions: a read-only command set exists"  claude-code/permissions.md 'built-in set of Bash commands as read-only'
chk "permissions: only ask/deny re-gate them"      claude-code/permissions.md 'add an `ask` or `deny` rule for it'
chk "cli: --permission-prompt-tool"                 claude-code/cli-reference.md 'permission-prompt-tool'
chk "cli: --bg"                                     claude-code/cli-reference.md '\-\-bg'
chk "cli: --worktree"                               claude-code/cli-reference.md '\-\-worktree'
chk "cli: --json-schema"                            claude-code/cli-reference.md 'json-schema'
chk "cli: --replay-user-messages"                   claude-code/cli-reference.md 'replay-user-messages'
chk "cli: --include-partial-messages"               claude-code/cli-reference.md 'include-partial-messages'
chk "sdk: AskUserQuestion via permission host"      claude-agent-sdk/user-input.md 'AskUserQuestion'
chk "headless: permission-prompt-tool"              claude-code/headless.md 'permission-prompt-tool'
chk "sessions: JSONL internal/unstable"             claude-code/sessions.md 'internal to Claude Code and changes'
chk "worktrees: .claude/worktrees default"          claude-code/worktrees.md '\.claude/worktrees'
chk "statusline: used_percentage"                   claude-code/statusline.md 'used_percentage'
chk "statusline: 300ms debounce"                    claude-code/statusline.md '300ms|300 ms'
chk "otel: http/json protocol"                      claude-code/monitoring-usage.md 'http/json'
chk "otel: tool_decision event"                     claude-code/monitoring-usage.md 'tool_decision'
chk "otel: api_request cost_usd"                    claude-code/monitoring-usage.md 'cost_usd'
chk "otel: session.id attribute"                    claude-code/monitoring-usage.md 'session\.id'
chk "otel: app.entrypoint"                          claude-code/monitoring-usage.md 'app\.entrypoint'
chk "otel: vcs repository attributes"               claude-code/monitoring-usage.md 'OTEL_METRICS_INCLUDE_REPOSITORY'
chk "otel: desktop / VS Code entrypoints"           claude-code/monitoring-usage.md 'claude-vscode|desktop'
chk "settings: env block"                           claude-code/settings-reference.md '"env"'
chk "sdk: settingSources default all"               claude-agent-sdk/typescript.md 'settingSources'
# These are documented on the site's v2 pages, which is where the claim that they were
# "v2, behind unstable_protocol_v2" came from. They are in the *schema's* ungated v1 module,
# advertised per agent as an initialize capability, and reachable on the SDK's default
# features — so they are unbuilt here, not unavailable. The SDK checks below pin that.
chk "acp: session/resume"                           acp/v2-session-list.md 'session/resume'
chk "acp: replayFrom"                               acp/v2-session-list.md 'replayFrom'
chk "acp: elicitation requestedSchema"              acp/v2-elicitation.md 'requestedSchema'
chk "acp: allow_always"                             acp/v2-tool-calls.md 'allow_always'
chk "acp: session/set_mode"                         acp/v1-session-modes.md 'session/set_mode'
chk "acp: terminal/create"                          acp/v1-terminals.md 'terminal/create'
chk "acp: fs/read_text_file"                        acp/v1-file-system.md 'fs/read_text_file'
chk "registry: claude-agent-acp"                    acp/registry.json 'claude-agent-acp'
chk "registry: codex-acp"                           acp/registry.json 'codex-acp'
chk "registry: opencode native"                     acp/registry.json '"opencode"'
chk "symphony: stall timeout"                       symphony/SPEC.md 'stall'
chk "codex app-server: JSON-RPC"                    codex/app-server-README.md 'JSON-RPC|thread/start'
chk "opencode openapi: /session"                    opencode/openapi.json '"/session"'

# D76: the SDK half of the claim above, checked against the installed crate source rather
# than its documentation — this is the claim that was wrong, so it is the one pinned hardest.
sdk=$(find "${CARGO_HOME:-$HOME/.cargo}/registry/src" -maxdepth 2 -type d -name 'agent-client-protocol-schema-*' 2>/dev/null | sort | tail -1)
# The provider surfaces added through 2026 that these notes now rest on. Each row was
# absent from the ledger while its page sat unread in concepts/reference/ — the gap R19 is about, and
# the reason the unit of verification is the row rather than the page.
chk "hooks: the prompt handler type exists"         claude-code/hooks.md '"type": "prompt"'
chk "hooks: the agent handler type exists"          claude-code/hooks.md '"type": "agent"'
chk "hooks: the mcp_tool handler type exists"       claude-code/hooks.md '"type": "mcp_tool"'
chk "hooks: ElicitationResult fires after an answer" claude-code/hooks.md 'ElicitationResult.*After a user responds'
chk "hooks: ConfigChange fires on a settings change" claude-code/hooks.md 'ConfigChange. *\| *When a configuration file changes'
chk "hooks: TaskCreated/TaskCompleted exist"        claude-code/hooks.md 'TaskCreated. *\| *When a task is being created'
chk "hooks: PreModelSwitch is before the switch"    claude-code/hooks.md 'PreModelSwitch. *\| *Before Claude Code applies a model switch'
chk "hooks: WorktreeRemove non-zero fails removal"  claude-code/hooks.md 'causes worktree removal to fail'
# /goal — the vendor's completion check, and the contrast D96 is built on.
chk "goal: a small fast model checks each turn"     claude-code/goal.md 'a small fast model checks whether the condition holds'
chk "goal: the evaluator runs no commands"          claude-code/goal.md "doesn't run commands or read files independently"
chk "goal: it is a prompt-based Stop hook"          claude-code/goal.md 'wrapper around a session-scoped'
# Skills — the prompt-template format adopted by D92, and the plugin channel of D99.
chk "skills: templates live in SKILL.md"            claude-code/skills.md 'SKILL\.md'
chk "skills: allowed-tools frontmatter"             claude-code/skills.md 'allowed-tools'
chk "skills: an argument-hint is declared"          claude-code/skills.md 'argument-hint'
chk "skills: a skill can fork into a subagent"      claude-code/skills.md '`context`'
# Deep links — the launch and handoff surface of D93.
chk "deep: claude-cli://open is the handler"        claude-code/deep-links.md 'claude-cli://open'
chk "deep: the prompt is filled, never sent"        claude-code/deep-links.md 'populated but not sent'
chk "deep: an external prompt is flagged"           claude-code/deep-links.md 'Prompt from an external link'
chk "deep: registration needs a first prompt"       claude-code/deep-links.md 'registers the .claude-cli://. handler'
chk "deep: VS Code opens a session by id"           claude-code/vs-code.md 'vscode://anthropic.claude-code/open'
# Auto mode — the second gate D94 reads back and D95/R20 take the warning from.
chk "auto: deny and ask precede the classifier"     claude-code/auto-mode-config.md 'evaluated before the classifier'
chk "auto: hard_deny blocks unconditionally"        claude-code/auto-mode-config.md 'hard_deny. rules block unconditionally'
chk "auto: project settings are not a source"       claude-code/auto-mode-config.md "doesn't read .autoMode. from project settings"
chk "auto: the effective config is printable"       claude-code/auto-mode-config.md 'claude auto-mode config'
chk "auto: PermissionDenied carries tool_input"     claude-code/auto-mode-config.md 'receives it as .tool_input'
# Scheduling and session mobility — the three standing-pipeline analogues, and --teleport.
chk "sched: /loop re-runs a prompt on a timer"      claude-code/scheduled-tasks.md 'Run a prompt repeatedly with'
chk "web: CLI session handoff is one-way"           claude-code/claude-code-on-the-web.md 'session handoff is one-way'

if [ -n "$sdk" ]; then
  sdkchk() { n=$((n+1)); if grep -q -E "$3" "$sdk/$2" 2>/dev/null; then printf 'OK   %s\n' "$1"; else printf 'MISS %s  (%s ~ /%s/)\n' "$1" "$2" "$3"; fail=1; fi; }
  sdkchk "sdk: the v1 module is ungated"            src/lib.rs '^pub mod v1;'
  sdkchk "sdk: only v2 is behind the feature"       src/lib.rs 'cfg\(feature = "unstable_protocol_v2"\)\]?\s*$'
  sdkchk "sdk: ResumeSessionRequest is v1"          src/v1/agent.rs 'pub struct ResumeSessionRequest'
  sdkchk "sdk: ListSessionsRequest is v1"           src/v1/agent.rs 'pub struct ListSessionsRequest'
  sdkchk "sdk: SetSessionModeRequest is v1"         src/v1/agent.rs 'pub struct SetSessionModeRequest'
  sdkchk "sdk: CreateElicitationRequest is v1"      src/v1/elicitation.rs 'pub struct CreateElicitationRequest'
  sdkchk "sdk: resume is an agent capability"       src/v1/agent.rs 'pub resume: Option<SessionResumeCapabilities>'
  # Devplane sends `session/resume` with no history replay and relies on that:
  # it kept the transcript itself, so a replay would write every line down twice.
  # `replayFrom` is a v2 field, and this is what says so.
  sdkchk "sdk: v1 resume carries no replayFrom"     src/v1/agent.rs 'pub struct ResumeSessionRequest'
  if grep -A 40 'pub struct ResumeSessionRequest' "$sdk/src/v1/agent.rs" 2>/dev/null | grep -q 'replay_from'; then
    printf 'MISS %s  (v1 ResumeSessionRequest grew a replay_from field)\n' "acp: v1 resume has no replayFrom"
    fail=1
  else
    printf 'OK   %s\n' "acp: v1 resume has no replayFrom"
  fi
  n=$((n+1))
else
  printf 'SKIP acp sdk source checks (crate not vendored; run cargo fetch)\n'
fi

# ── Counts, computed from the corpus rather than grepped from prose ──────────────
# The whole ledger above asks "does the source say this", which cannot catch a number
# these notes invented about somebody else's system. Four places said the ACP registry
# held 51 agents while concepts/reference/acp/registry.json said 41, and nothing failed, because a
# count is not a sentence to grep for. A number about external data is now DERIVED here
# and the notes are checked against it.
countchk() { # label  actual  file-glob-in-concepts  regex-with-one-capture
  n=$((n+1))
  local claimed
  # QUALITY.md and DECISIONS.md are excluded on purpose: their job is to record claims that
  # turned out to be wrong, so they have to be able to quote the wrong number. Everywhere
  # else, a figure about an external system is an assertion and is checked. (Found the first
  # time this ran: the row describing the "51 agents" mistake failed the check for it.)
  claimed=$(grep -rhoE "$4" $(ls ../*.md | grep -vE 'QUALITY|DECISIONS') 2>/dev/null | grep -oE '[0-9]+' | sort -u | tr '\n' ' ' | sed 's/ $//')
  if [ -z "$claimed" ]; then
    printf 'SKIP %s (concepts/ not present)\n' "$1"
  elif [ "$claimed" = "$2" ]; then
    printf 'OK   %s = %s\n' "$1" "$2"
  else
    printf 'MISS %s: corpus says %s, concepts/ says %s\n' "$1" "$2" "$claimed"; fail=1
  fi
}
# ── Which tools a rule reaches: the vendor's own rule-format table (D182) ─────
chk "cc: a Bash rule also governs Monitor"        claude-code/tools-reference.md '\`Bash\(npm run \*\)\`[^|]*\| Bash, Monitor'
chk "cc: a Read rule also governs LSP"            claude-code/tools-reference.md '\`Read\(~/secrets/\*\*\)\`[^|]*\| Read, Grep, Glob, LSP'
chk "cc: an Edit rule governs three writers"      claude-code/tools-reference.md '\`Edit\(/src/\*\*\)\`[^|]*\| Edit, Write, NotebookEdit'
chk "cc: PowerShell has its own rule syntax"      claude-code/tools-reference.md '\`PowerShell\(Get-ChildItem \*\)\`'
chk "cc: Monitor runs a command in background"    claude-code/tools-reference.md 'Runs a command in the background'
chk "cc: LSP reads files through a language server" claude-code/tools-reference.md 'code intelligence from a running language server'
# ── PowerShell is a dialect, not a spelling of Bash (D183) ────────────────────
chk "ps: aliases are canonicalized first"         claude-code/permissions.md 'aliases are canonicalized before matching'
chk "ps: a cmdlet rule matches its aliases"       claude-code/permissions.md 'matches \`gci\`, \`ls\`, and \`dir\`'
chk "ps: matching ignores case"                   claude-code/permissions.md 'Matching is case-insensitive'
chk "ps: compound commands split like Bash"       claude-code/permissions.md 'A rule must match every subcommand'
chk "ps: rules use the Bash rule shape"           claude-code/permissions.md 'PowerShell permission rules use the same shape as Bash rules'
# ── Why an interpreter grant is overbroad *there* as well as here (D200) ──────
# `devplane check` tells people `Bash(python:*)` approves `python -c '…'` and
# that Claude Code reads it the same way. That second half is a claim about
# somebody else's product, published in our own README, so it is pinned to the
# sentence it follows from rather than left as a reading.
chk "cc: :* is a trailing wildcard"              claude-code/permissions.md 'The `:\*` suffix is an equivalent way to write a trailing wildcard'
chk "cc: a trailing wildcard takes any argument" claude-code/permissions.md 'npm run test --watch'
# ── What a path deny does NOT reach, stated by the vendor ─────────────────────
chk "cc: a deny misses an interpreter's own reads" claude-code/permissions.md 'like a Python or Node script that opens files itself'
# ── Spec-driven development: the category's analysers report and never decide (D178) ─
chk "sdd: speckit analyze is non-destructive"     sdd/spec-kit-analyze.md 'non-destructive cross-artifact consistency and quality analysis'
chk "sdd: speckit analyze writes nothing"         sdd/spec-kit-analyze.md 'Do \*\*not\*\* modify any files'
chk "sdd: its findings carry a severity"          sdd/spec-kit-analyze.md 'CRITICAL'
chk "sdd: speckit converge appends, never fails"  sdd/spec-kit-converge.md 'append any remaining unbuilt work as new tasks to tasks.md'
chk "sdd: the extension hook has no executor"     sdd/spec-kit-analyze.md 'leave condition evaluation to the HookExecutor implementation'
chk "sdd: a mandatory hook is optional: false"    sdd/spec-kit-analyze.md 'optional: false'
chk "sdd: the analyser calls itself read-only"    sdd/spec-kit-analyze.md 'STRICTLY READ-ONLY'
chk "sdd: requirements carry stable ids"          sdd/spec-kit-analyze.md 'FR-###.*SC-###|FR-/SC- identifiers'
# The shape `--spec` actually reads. Not the section names, which differ per
# tool and are deliberately not recognised — the folder, and the task list.
chk "sdd: a specification is a folder per feature" sdd/spec-kit-tasks-template.md '/specs/\[###-feature-name\]/'
chk "sdd: its progress is a markdown task list"   sdd/spec-kit-tasks-template.md '^- \[ \] T[0-9]+'
chk "sdd: openspec writes a task list too"        sdd/openspec-README.md 'tasks\.md.*implementation checklist'
chk "sdd: openspec's unit is a change folder"     sdd/openspec-README.md 'openspec/changes/'
chk "sdd: NEEDS CLARIFICATION is spec kit's word" sdd/spec-kit-spec-template.md '\[NEEDS CLARIFICATION'
# ── AGENTS.md: the one context file every vendor reads (D179) ────────────────
chk "agents.md: stewarded by the AAIF"            standards/agents-md.md 'Agentic AI Foundation'
chk "agents.md: over 60k repositories"            standards/agents-md.md 'over.{0,40}60k'
chk "agents.md: it mandates no structure"         standards/agents-md.md 'the agent simply parses the text you provide'
if [ -d .. ]; then
  acp_agents=$(python3 -c 'import json;print(len(json.load(open("acp/registry.json"))["agents"]))' 2>/dev/null || echo '?')
  # Every spelling these notes use for the registry size, and only those: a loose pattern
  # picks up "21 agent permission systems" from a cited paper and reports a false miss,
  # which is how a check stops being read.
  countchk "acp registry agent count" "$acp_agents" '' \
    '[0-9]+ registry agents|registry of [0-9]+|registry[^.]{0,40}: [0-9]+ agents|one client for [0-9]+ agents|\*\*[0-9]+\*\* agents'
fi

echo "verify-claims: $n claims checked, exit $fail"
exit $fail

#!/usr/bin/env bash
# Records one pinned ACP adapter's own output, for the replay tests.
#
#   scripts/record-acp.sh <agent-id>      # claude | codex | copilot | gemini
#
# The adapter is the registry's pinned command, read from the recorded registry
# copy (tests/fixtures/acp/registry.json), wrapped in scripts/acp-tee.sh. A small
# scripted client drives two sessions in a scratch git repository:
#
#   1. the adapter started in its ask-first mode through its own documented
#      start configuration (Codex: INITIAL_AGENT_MODE=read-only; the others:
#      nothing set), prompted to run `touch devplane-replay-marker`, every
#      permission it asks answered with its reject option — so nothing the
#      recording asks for runs, and the recording does not depend on any
#      client's rule matching;
#   2. the adapter with nothing configured, opened and closed without a
#      prompt, to observe where it starts on its own.
#
# Written to tests/fixtures/acp/<registry-id>@<version>/: session.jsonl (the
# first session), unconfigured.jsonl (the second), expect.json (the call whose
# merged input carries the marker command, computed from the transcript) and
# SOURCE (date, command, OS, prompt, capabilities, rewrites, and whether the
# call's input arrived in a later update). Absolute paths become /work, session
# ids s-1, and sign-in details and token-shaped values are dropped.
#
# Needs the adapter signed in on this machine and the network. Never run by
# the tests, by CI or by the host. Recordings are re-made, never edited.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
agent=${1:-}
case "$agent" in
  claude)  registry_id=claude-acp ;;
  codex)   registry_id=codex-acp ;;
  copilot) registry_id=github-copilot-cli ;;
  gemini)  registry_id=gemini ;;
  *) echo "usage: $0 claude|codex|copilot|gemini" >&2; exit 2 ;;
esac

registry="$root/tests/fixtures/acp/registry.json"
[[ -f $registry ]] || { echo "no registry copy at $registry; run: just pins --write" >&2; exit 2; }
package=$(jq -r --arg id "$registry_id" '.agents[] | select(.id==$id) | .distribution.npx.package' "$registry")
args=$(jq -r --arg id "$registry_id" '.agents[] | select(.id==$id) | (.distribution.npx.args // []) | join(" ")' "$registry")
version=$(jq -r --arg id "$registry_id" '.agents[] | select(.id==$id) | .version' "$registry")
[[ -n $package && $package != null ]] || { echo "$registry_id has no npx entry in the registry copy" >&2; exit 2; }

command="npx -y $package${args:+ $args}"
ask_first_env=()
start_note="nothing set (the adapter starts in its ask-first mode on its own)"
if [[ $agent == codex ]]; then
  ask_first_env=(INITIAL_AGENT_MODE=read-only)
  start_note="INITIAL_AGENT_MODE=read-only"
fi

out="$root/tests/fixtures/acp/$registry_id@$version"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
work="$scratch/work"
mkdir -p "$work"
git -C "$work" init -q
printf '[policy]\ndeny = ["Bash(touch devplane-replay-marker*)"]\n' > "$work/devplane.toml"
git -C "$work" add -A && git -C "$work" -c user.email=r@example.invalid -c user.name=recorder commit -qm init

prompt='Run the shell command `touch devplane-replay-marker` and then stop.'

# A recording made from inside another agent's session would inherit its
# variables; the adapter is started as if from a plain terminal.
clean_env=(env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT -u CLAUDE_CODE_EXECPATH -u CLAUDE_CODE_SSE_PORT)

echo "recording $registry_id@$version: $command" >&2
"${clean_env[@]}" ${ask_first_env[@]+"${ask_first_env[@]}"} python3 "$root/scripts/acp-drive.py" \
  --cwd "$work" --prompt "$prompt" --out "$scratch/session.raw.jsonl" -- \
  "$root/scripts/acp-tee.sh" "$scratch/session.raw.jsonl" $command
"${clean_env[@]}" python3 "$root/scripts/acp-drive.py" \
  --cwd "$work" --out "$scratch/unconfigured.raw.jsonl" -- \
  "$root/scripts/acp-tee.sh" "$scratch/unconfigured.raw.jsonl" $command

mkdir -p "$out"
python3 "$root/scripts/acp-drive.py" --rewrite "$scratch" --home "$HOME" \
  --in "$scratch/session.raw.jsonl" --to "$out/session.jsonl"
python3 "$root/scripts/acp-drive.py" --rewrite "$scratch" --home "$HOME" \
  --in "$scratch/unconfigured.raw.jsonl" --to "$out/unconfigured.jsonl"
later=$(python3 "$root/scripts/acp-drive.py" --expect "$out/session.jsonl" --to "$out/expect.json")

caps=$(jq -c 'select(.dir=="in" and .msg.method=="initialize") | .msg.params.clientCapabilities' "$out/session.jsonl" | head -1)
cat > "$out/SOURCE" <<EOF
Recorded $(date -u +%Y-%m-%d) by scripts/record-acp.sh $agent, on $(uname -sr).
Adapter: $command (the registry's version $version for $registry_id).
Ask-first start: $start_note.
Prompt: $prompt
Client capabilities declared: $caps
Every permission the adapter asked was answered with its reject option.
Rewrites: the scratch directory and the home directory became /work; every
session id became s-1; sign-in method details and token-shaped values were
dropped. Nothing else was changed.
Later-input update observed: $later
unconfigured.jsonl is a second session opened with nothing configured and no
prompt, to observe where the adapter starts on its own.
EOF
echo "wrote $out" >&2

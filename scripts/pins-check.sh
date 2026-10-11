#!/usr/bin/env bash
# Compares the built-in npm adapter pins with the live ACP registry index.
#
#   scripts/pins-check.sh            # print pin / registry / current|behind
#   scripts/pins-check.sh --write    # and replace the recorded registry copy
#
# Exit 0 when every pin is current, 1 when any is behind, 2 when the registry
# cannot be reached or read. A maintainer's command (`just pins`): never part
# of `just verify`, CI or the host, because it needs the network.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
url=https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json
copy="$root/tests/fixtures/acp/registry.json"

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
if ! curl -sf --max-time 30 "$url" -o "$tmp"; then
  echo "the registry could not be reached: $url" >&2
  exit 2
fi
if ! jq -e '.agents | length > 0' "$tmp" >/dev/null 2>&1; then
  echo "the registry answered with something that is not its index" >&2
  exit 2
fi

behind=0
# The built-in pins, as `builtin()` in src/acp/agent.rs spells them:
# `npx -y <package>@<x.y.z>`.
while read -r spec; do
  name=${spec%@*}
  pin=${spec##*@}
  entry=$(jq -r --arg n "$name" '.agents[] | select((.distribution.npx.package // "") | startswith($n + "@")) | "\(.id) \(.distribution.npx.package)"' "$tmp" | head -1)
  if [[ -z $entry ]]; then
    printf '%-44s pinned %-9s registry %-9s %s\n' "$name" "$pin" "-" "not in the registry"
    behind=1
    continue
  fi
  id=${entry%% *}
  registry=${entry##*@}
  state=current
  [[ $pin == "$registry" ]] || { state=behind; behind=1; }
  printf '%-20s %-44s pinned %-9s registry %-9s %s\n' "$id" "$name" "$pin" "$registry" "$state"
done < <(awk '/^pub fn builtin\(\)/,/^}/' "$root/src/acp/agent.rs" |
  grep -oE '"npx -y [^" ]+@[0-9]+\.[0-9]+\.[0-9]+' | sed 's/^"npx -y //')

if [[ ${1:-} == --write ]]; then
  mkdir -p "$(dirname "$copy")"
  cp "$tmp" "$copy"
  cat > "$(dirname "$copy")/registry.SOURCE" <<EOF
registry.json: the ACP registry index, byte for byte as fetched from
$url
on $(date -u +%Y-%m-%dT%H:%MZ) with curl, by scripts/pins-check.sh --write.
Replaced by running that again; never edited by hand.
EOF
  echo "replaced $copy" >&2
fi
exit $behind

#!/usr/bin/env bash
# A report, not a check (always exits 0): vendor changelog rows touching the channels
# Devplane reads — hooks, permission modes, settings that decide whether a hook runs.
#
#   scripts/changelog-rows.sh --channels   # the rows, newest first
#   scripts/changelog-rows.sh --fetch      # refresh the changelog, then report
#
# Reads the gitignored `concepts/reference/claude-code/CHANGELOG.md`.
set -u
cd "$(dirname "$0")/.." || exit 1

CHANGELOG="concepts/reference/claude-code/CHANGELOG.md"
CHANGELOG_URL="https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md"
MODE="${1:---channels}"

case "$MODE" in
  --fetch)
    curl -fsSL "$CHANGELOG_URL" -o "$CHANGELOG" \
      || { echo "changelog-rows: could not fetch $CHANGELOG_URL"; exit 2; } ;;
  --channels) ;;
  *) echo "usage: $0 [--channels|--fetch]"; exit 2 ;;
esac

[ -f "$CHANGELOG" ] || {
  echo "changelog-rows: $CHANGELOG is missing — run scripts/fetch-reference.sh"
  exit 2
}

# In if a row changes what a rule can say or which calls it reaches; out if it changes
# how a person is asked. `sandbox` is deliberately out.
CHANNELS='allowed_domains|allowedDomains|defaultMode|bypassPermissions|PreToolUse|PostToolUse|PreModelSwitch'
# Rule-syntax rows are the vendor's business and are left out.
RULES='deny rule|allow rule|ask rule|permission rule|permission check|auto-approv|Bash\(|Read\(|Edit\(|Write\(|--disallowedTools|--allowedTools|permission mode'

channel_rows() {
  awk '/^## / { v = substr($0, 4); next }
       /^- / { if (v != "") printf "%s\t%s\n", v, substr($0, 3) }' "$CHANGELOG" \
    | grep -E "$CHANNELS" | grep -vE "$RULES"
}

# The cache's age comes first: a report over a stale cache looks like a fresh one.
newest=$(grep -m1 -oE '^## [0-9]+\.[0-9]+\.[0-9]+' "$CHANGELOG" | awk '{print $2}')
age_days=$(( ( $(date +%s) - $(stat -f %m "$CHANGELOG" 2>/dev/null || stat -c %Y "$CHANGELOG") ) / 86400 ))
echo "changelog-rows: corpus is at ${newest:-unknown}, fetched ${age_days}d ago — \`$0 --fetch\` to refresh"
echo "channel rows — what the vendor changed about the hooks and modes Devplane reads (a report, not a check)"
echo "boundary: sandbox is excluded deliberately; it changes the environment a call runs in"
echo
channel_rows | while IFS="$(printf '\t')" read -r v text; do
  printf "  %-9s %s\n" "$v" "$(printf "%s" "$text" | cut -c1-140)"
done
echo
echo "changelog-rows: $(channel_rows | wc -l | tr -d ' ') channel row(s)"
exit 0

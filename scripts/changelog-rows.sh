#!/usr/bin/env bash
# Every changelog row that could change what a permission rule decides, and
# what was done about it.
#
# The other checks cannot close this hole. `verify-claims.sh` asks whether a
# *reference page* still says something, which is a different property from
# whether it is true — and twice now the page was fetched, current, and wrong
#. `verify-permissions-diff.sh` asks a running Claude Code about the
# shapes this project thought to generate, which is a guess about what an agent
# writes. What actually found eight widenings in one sitting was a person
# reading thirty changelog rows and typing the forms they name into
# `vibeplane explain` — and a person reading is a habit with a base rate, not a
# guarantee.
#
# So this makes the reading mechanical in the only half a machine can do: it
# **finds** the rows and **fails until every one is accounted for**. It cannot
# write the probe — turning "deny rules missing option values
# (`--ignore-revs-file=.env`)" into a runnable case needs a person — but it can
# guarantee that nobody silently skipped the row that would have told them to.
#
#   scripts/changelog-rows.sh          # check the ledger is complete
#   scripts/changelog-rows.sh --new    # print the unaccounted rows, ready to paste
#
# The ledger is `scripts/changelog-ledger.txt`. Two dispositions, and the second
# is as valuable as the first:
#
#   case     — a test or a harness shape covers it. Name what.
#   declined — it cannot change a verdict here. Say why.
#
# A declined row is a decision; an unread row is a gap wearing a decision's
# clothes.
set -u
cd "$(dirname "$0")/.." || exit 1

CHANGELOG="specs/claude-code/CHANGELOG.md"
LEDGER="scripts/changelog-ledger.txt"
MODE="${1:-check}"
CHANGELOG_URL="https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md"

# `--fetch` re-downloads the one file this script reads. It is separate from
# `fetch-specs.sh`, which re-downloads all 130 and needs a minute.
#
# This exists because the script was green and wrong. It reported "41 rule rows,
# all accounted for" while measuring a corpus pinned two releases behind the
# vendor; one `curl` over this single file turned the same unchanged script into
# four unaccounted rows and three reproducible defects. **A checker over a cached
# corpus reports on the cache**, and its green is indistinguishable from the real
# thing — so the cache's age is printed on every run, whatever the verdict.
if [ "$MODE" = "--fetch" ]; then
  curl -fsSL "$CHANGELOG_URL" -o "$CHANGELOG" || { echo "changelog-rows: could not fetch $CHANGELOG_URL"; exit 2; }
  MODE=check
fi

[ -f "$CHANGELOG" ] || {
  echo "changelog-rows: $CHANGELOG is missing — run scripts/fetch-specs.sh"
  exit 2
}
[ -f "$LEDGER" ] || { echo "changelog-rows: $LEDGER is missing"; exit 2; }

FLOOR="$(grep -m1 '^floor:' "$LEDGER" | awk '{print $2}')"
[ -n "$FLOOR" ] || { echo "changelog-rows: $LEDGER has no 'floor:' line"; exit 2; }

# What counts as a row that could change a verdict.
#
# Deliberately about **rule semantics** rather than about permissions at large:
# a dialog, a telemetry field or an auto-mode prose rule cannot make
# `never_auto = ["Read(.env)"]` stop or start firing, and a filter that pulls
# them in produces a ledger nobody finishes. The filter is itself a guess, and
# the thing that backstops a wrong guess is the differential harness rather than
# a wider grep here.
RULES='deny rule|allow rule|ask rule|permission rule|permission check|auto-approv|Bash\(|Read\(|Edit\(|Write\(|--disallowedTools|--allowedTools|permission mode'

# Rows since the floor, newest first, as `<version>\t<text>`.
rows() {
  awk -v floor="## $FLOOR" '
    $0 == floor { exit }
    /^## / { v = substr($0, 4); next }
    /^- / { if (v != "") printf "%s\t%s\n", v, substr($0, 3) }
  ' "$CHANGELOG" | grep -E "$RULES"
}

# A row is accounted for when its text appears in the ledger. Matched on the
# whole text on purpose: a row the vendor edits becomes unaccounted and is read
# again, which is the behaviour that is wanted.
missing=0
new=""
while IFS=$'\t' read -r version text; do
  [ -n "$version" ] || continue
  if ! grep -Fq -- "$text" "$LEDGER"; then
    missing=$((missing + 1))
    new="$new$version | ???????? | $text"$'\n'
  fi
done < <(rows)

total=$(rows | wc -l | tr -d ' ')

if [ "$MODE" = "--new" ]; then
  printf '%s' "$new"
  exit 0
fi

newest=$(grep -m1 -oE '^## [0-9]+\.[0-9]+\.[0-9]+' "$CHANGELOG" | awk '{print $2}')

# ── The clock ────────────────────────────────────────────────────────────────
#
# `--owed` answers one question with an exit code: **is anything owed?** The
# vendor ships most days and this ledger is cleared when somebody remembers, so
# the gap between the two is where every silent widening has been born. A cron
# line on the machine with the signed-in agent runs this and says so.
#
# `--advance` moves the cheap floor, and **only on a green run**: a constant a
# person edits is a claim about attention, and this one is supposed to be a
# claim about measurement. Nothing here touches `VERIFIED_AGAINST`, which moves
# only when the full differential matrix runs green and costs real money.
POLICY="src/core/policy.rs"
cleared=$(grep -oE 'pub const ROWS_CLEARED_THROUGH: &str = "[0-9.]+"' "$POLICY" \
  | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')

if [ "$MODE" = "--owed" ] || [ "$MODE" = "--advance" ]; then
  if [ -z "$newest" ] || [ -z "$cleared" ]; then
    echo "changelog-rows: cannot read the vendor head or $POLICY"; exit 2
  fi
  if [ "$newest" = "$cleared" ] && [ "$missing" -eq 0 ]; then
    echo "changelog-rows: nothing owed — rows cleared through $cleared, which is the vendor's head"
    exit 0
  fi
  behind=$(awk -v a="$newest" -v b="$cleared" 'BEGIN{
    split(a,x,"."); split(b,y,".");
    print (x[1]==y[1] && x[2]==y[2]) ? x[3]-y[3] : "?" }')
  if [ "$MODE" = "--advance" ]; then
    if [ "$missing" -ne 0 ]; then
      echo "changelog-rows: $missing rows are not accounted for — the floor does not move on a red run"
      exit 1
    fi
    sed -i.bak "s/pub const ROWS_CLEARED_THROUGH: &str = \"$cleared\"/pub const ROWS_CLEARED_THROUGH: \&str = \"$newest\"/" "$POLICY"
    rm -f "$POLICY.bak"
    echo "changelog-rows: rows cleared through $cleared -> $newest ($behind release(s)), ledger green"
    echo "  now update STATE.md to match, which \`concepts-check.sh\` will insist on"
    exit 0
  fi
  echo "changelog-rows: $behind release(s) owed — rows cleared through $cleared, vendor is at $newest"
  [ "$missing" -eq 0 ] || echo "changelog-rows: and $missing rule row(s) in the gap are not accounted for"
  exit 1
fi
age_days=$(( ( $(date +%s) - $(stat -f %m "$CHANGELOG" 2>/dev/null || stat -c %Y "$CHANGELOG") ) / 86400 ))
echo "changelog-rows: corpus is at ${newest:-unknown}, fetched ${age_days}d ago — \`$0 --fetch\` to refresh"
[ "${age_days:-0}" -lt 7 ] || echo "changelog-rows: WARNING — a week-old corpus makes a clean run mean very little"

if [ "$missing" -eq 0 ]; then
  echo "changelog-rows: $total rule rows since $FLOOR, all accounted for"
  exit 0
fi

echo "changelog-rows: $missing of $total rule rows since $FLOOR are not accounted for."
echo
echo "Read each one, decide whether it can change a verdict here, and add a line"
echo "to $LEDGER with 'case <what covers it>' or 'declined <why not>':"
echo
printf '%s' "$new"
exit 1

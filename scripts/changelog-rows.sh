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

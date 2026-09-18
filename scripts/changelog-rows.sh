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
# `devplane explain` — and a person reading is a habit with a base rate, not a
# guarantee.
#
# So this makes the reading mechanical in the only half a machine can do: it
# **finds** the rows and **fails until every one is accounted for**. It cannot
# write the probe — turning "deny rules missing option values
# (`--ignore-revs-file=.env`)" into a runnable case needs a person — but it can
# guarantee that nobody silently skipped the row that would have told them to.
#
# **Half of that sentence stopped being true on 2026-09-18, and which half
# matters.** Writing a probe is still a person's job and always will be: prose
# does not become a runnable call by being parsed harder. What became mechanical
# is *running* it. A row dispositioned `probe` names its shape by an identifier,
# so the shape can be re-run against a new release and dated — which is the
# difference between a floor and the word "measured" typed into a comment.
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

CHANGELOG="concepts/reference/claude-code/CHANGELOG.md"
LEDGER="scripts/changelog-ledger.txt"
MODE="${1:-check}"
CHANGELOG_URL="https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md"

# `--fetch` re-downloads the one file this script reads. It is separate from
# `fetch-reference.sh`, which re-downloads all 130 and needs a minute.
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
  echo "changelog-rows: $CHANGELOG is missing — run scripts/fetch-reference.sh"
  exit 2
}
# The channel ledger needs the changelog and nothing else. The *rule* ledger it
# used to sit beside is gone with the matcher it fed: Devplane no longer mirrors
# anybody's permission semantics, so a changed rule shape is the vendor's
# business. A changed hook contract is still ours, and that is what this reads.
if [ "$MODE" != "--channels" ]; then
  [ -f "$LEDGER" ] || { echo "changelog-rows: $LEDGER is missing"; exit 2; }
fi

FLOOR="$(grep -m1 '^floor:' "$LEDGER" 2>/dev/null | awk '{print $2}')"
if [ "$MODE" != "--channels" ] && [ -z "$FLOOR" ]; then
  echo "changelog-rows: $LEDGER has no 'floor:' line"; exit 2
fi

# What counts as a row that could change a verdict.
#
# Deliberately about **rule semantics** rather than about permissions at large:
# a dialog, a telemetry field or an auto-mode prose rule cannot make
# `never_auto = ["Read(.env)"]` stop or start firing, and a filter that pulls
# them in produces a ledger nobody finishes. The filter is itself a guess, and
# the thing that backstops a wrong guess is the differential harness rather than
# a wider grep here.
RULES='deny rule|allow rule|ask rule|permission rule|permission check|auto-approv|Bash\(|Read\(|Edit\(|Write\(|--disallowedTools|--allowedTools|permission mode'

# ---------------------------------------------------------------------------
# **The second ledger: the channels, not the rules.**
#
# `RULES` above reads `Bash,` and not `Bash(`, which is how 2.1.271's
# per-command `allowed_domains` on Bash, PowerShell and Monitor — a
# permission-shaped construct — went unseen. The fix is *not* to widen `RULES`:
# its count is quoted in the notes and load-bearing, and widening it in place
# would change what that number means without saying so.
#
# **The boundary was measured before it was chosen.** Everything
# permission-adjacent matches 220 rows; adding `sandbox` alone takes a candidate
# from 41 to 142. An unfinished ledger is worse than none, because it looks like
# coverage.
#
# So: **in scope if a row changes what a rule can say or which calls a rule
# reaches; out if it changes how a person is asked.** `sandbox` is the arguable
# exclusion and is excluded deliberately — it changes the environment a call
# runs in rather than what a rule can say. That line is arguable, which is why
# it is written here rather than left implicit in the pattern.
CHANNELS='allowed_domains|allowedDomains|defaultMode|bypassPermissions|PreToolUse|PostToolUse|PreModelSwitch'

# The channel rows, newest first, excluding anything the rule ledger already has.
channel_rows() {
  awk '/^## / { v = substr($0, 4); next }
       /^- / { if (v != "") printf "%s\t%s\n", v, substr($0, 3) }' "$CHANGELOG" \
    | grep -E "$CHANNELS" | grep -vE "$RULES"
}

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

# ── The second owed count, and why it is not the first ───────────────────────
#
# `missing` is what the **read** floor owes: rows nobody has dispositioned at
# all. This is what the **measured** floor owes: rows dispositioned `case`,
# whose evidence is a test over our own matcher and therefore says nothing about
# the vendor. Two counts, never one — they are owed to two different claims and
# a single number would hide which.
PROBES="scripts/probes.txt"
owed_measured=$(grep -cE '^[0-9]+\.[0-9]+\.[0-9]+ \| case \|' "$LEDGER" || true)

# A `probe:<id>` that names nothing runnable is an error, not a silent skip.
# The failure this whole layer exists to prevent is a check that stops checking
# and stays green, so an unresolvable id fails rather than being ignored.
bad_ids=""
if [ -f "$PROBES" ]; then
  while read -r id; do
    [ -n "$id" ] || continue
    grep -qE "^$id[[:space:]]*\|" "$PROBES" || bad_ids="$bad_ids $id"
  done < <(grep -oE '\^ probe:[a-z0-9][a-z0-9_-]{2,63}' "$LEDGER" | sed 's/^\^ probe://' | sort -u)
else
  echo "changelog-rows: $PROBES is missing — a probe id cannot be checked against anything"
  fail_probes=1
fi
if [ -n "$bad_ids" ]; then
  echo "changelog-rows: the ledger cites probes that $PROBES does not declare:$bad_ids"
  fail_probes=1
fi

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

# `--channels` prints the second ledger and says how big it is, so the boundary
# stays a number somebody can argue with rather than a claim.
if [ "$MODE" = "--channels" ]; then
  n=$(channel_rows | wc -l | tr -d " ")
  echo "channel ledger — rows that change what a rule can say or which calls it reaches"
  echo "boundary: sandbox is excluded deliberately; it changes the environment a call runs in"
  echo
  channel_rows | while IFS="$(printf "\t")" read -r v text; do
    printf "  %-9s %s\n" "$v" "$(printf "%s" "$text" | cut -c1-140)"
  done
  echo
  echo "changelog-rows: $n channel row(s): what the vendor changed about the hooks and modes Devplane uses"
  exit 0
fi

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

# Two owed counts, reported apart, because they are owed to two different
# claims. Collapsing them would be the same mistake as collapsing the floors
# they answer to, one layer down.
probed=$(grep -cE '^[0-9]+\.[0-9]+\.[0-9]+ \| probe \|' "$LEDGER" || true)
if [ "$owed_measured" -eq 0 ]; then
  echo "changelog-rows: measured floor — nothing owed; $probed row(s) name a probe"
else
  echo "changelog-rows: measured floor — $owed_measured row(s) owed (dispositioned \`case\`: a test over our own matcher, which says nothing about the vendor); $probed row(s) name a probe"
fi

if [ -n "${fail_probes:-}" ]; then
  exit 1
fi

if [ "$missing" -eq 0 ]; then
  echo "changelog-rows: read floor — $total rule rows since $FLOOR, all accounted for"
  exit 0
fi

echo "changelog-rows: $missing of $total rule rows since $FLOOR are not accounted for."
echo
echo "Read each one, decide whether it can change a verdict here, and add a line"
echo "to $LEDGER with 'probe <id from scripts/probes.txt>', 'case <what covers it>'"
echo "or 'declined <why not>':"
echo
printf '%s' "$new"
exit 1

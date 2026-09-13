#!/usr/bin/env bash
# Holds concepts/ to its own rules: every cross-file link resolves, every D/R id is unique,
# every file carries the header block, and no file but ROADMAP.md records unfinished work.
# A recipe rather than a CI stage because concepts/ is untracked.
set -u
cd "$(dirname "$0")/../concepts" || { echo "no concepts/ directory"; exit 1; }
fail=0
for f in *.md; do
  # links
  grep -o '\]([A-Z_]*\.md\(#[a-z0-9-]*\)\?)' "$f" | sed 's/^](//; s/)$//; s/#.*//' | sort -u | while read -r t; do
    [ -f "$t" ] || { echo "$f: broken link -> $t"; exit 9; }
  done || fail=1
  # header
  [ "$f" = README.md ] || grep -q '^> Part of the Vibeplane architecture notes' "$f" || { echo "$f: missing header block"; fail=1; }
  # unfinished-work markers outside the roadmap
  [ "$f" = ROADMAP.md ] || ! grep -q -E '^\s*- \[ \]|\bTODO\b|\bTBD\b' "$f" || { echo "$f: records unfinished work (belongs in ROADMAP.md)"; fail=1; }
done
# id uniqueness
for pair in "D DECISIONS.md" "R RISKS.md"; do
  set -- $pair
  dup=$(grep -o "^| $1[0-9]\+ " "$2" | sort | uniq -d)
  [ -z "$dup" ] || { echo "$2: duplicate ids: $dup"; fail=1; }
done
# every cited D/R id exists
cited=$(grep -oh '\bD[0-9]\+\b' *.md | sort -u); defined=$(grep -o '^| D[0-9]\+ ' DECISIONS.md | tr -d '| ' | sort -u)
for id in $cited; do echo "$defined" | grep -qx "$id" || { echo "cited but undefined: $id"; fail=1; }; done
cited=$(grep -oh '\bR[0-9]\+\b' *.md | sort -u); defined=$(grep -o '^| R[0-9]\+ ' RISKS.md | tr -d '| ' | sort -u)
for id in $cited; do echo "$defined" | grep -qx "$id" || { echo "cited but undefined: $id"; fail=1; }; done
[ $fail = 0 ] && echo "concepts-check: ok ($(ls *.md | wc -l | tr -d ' ') files)"
exit $fail

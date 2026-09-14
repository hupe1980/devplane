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
# Nothing in the published tree may cite a D or R id. These notes are gitignored, so a
# `(D34)` in a doc comment renders on docs.rs — and on the documentation site — as a
# reference to a document the reader cannot open. The rule was written here from the
# start and nothing enforced it; five citations had leaked into the crate.
leaked=$(cd .. && grep -rnoE '\((D|R)[0-9]+(, ?(D|R)[0-9]+)*\)' \
  crates site README.md 2>/dev/null | grep -v '/target/')
if [ -n "$leaked" ]; then
  echo "published tree cites an internal decision id:"
  echo "$leaked" | sed 's/^/  /'
  fail=1
fi

# Every `vibeplane.toml` example has to be one the parser accepts. `deny_unknown_fields`
# means a single designed-but-unbuilt key fails the whole file — and takes that repository's
# permission rules down with it — so a reference that cannot be pasted is worse than none.
# It has happened twice: five sections that did not exist, and an `on =` key for standing
# pipelines. `tests/documentation.rs` checks both these notes and the README; it skips the
# notes when they are absent, which is why it is worth running from here as well.
if command -v cargo >/dev/null 2>&1; then
  ( cd .. && cargo test --quiet --test documentation ) >/dev/null 2>&1 \
    || { echo "a vibeplane.toml example does not parse (cargo test --test documentation)"; fail=1; }
fi

# The storage decision carries numbers, so the numbers are recomputed rather than
# trusted (D70, D78).
if command -v cargo >/dev/null 2>&1; then
  ../scripts/deps-count.sh >/dev/null 2>&1 \
    || { echo "the dependency figures in D70/D78 have drifted (scripts/deps-count.sh)"; fail=1; }
fi

# The test count is an internal figure and internal figures were supposed to be the safe
# kind — and this one still disagreed with itself across two documents for a pass (366 in
# the roadmap, 377 in the quality notes) because nothing recomputed it. The static count of
# test attributes matches what `cargo test` reports exactly, so the check costs a grep
# rather than a build.
tests_in_tree=$(grep -rhE "^[[:space:]]*#\[(tokio::)?test\]" ../src ../tests 2>/dev/null | wc -l | tr -d ' ')
if [ "${tests_in_tree:-0}" -gt 0 ]; then
  claimed=$(grep -rhoE '\*\*[0-9]+ tests\*\*' *.md | grep -oE '[0-9]+' | sort -u | tr '\n' ' ' | sed 's/ $//')
  if [ -n "$claimed" ] && [ "$claimed" != "$tests_in_tree" ]; then
    echo "the test count has drifted: the tree has $tests_in_tree, these notes say $claimed"; fail=1
  fi
fi

[ $fail = 0 ] && echo "concepts-check: ok ($(ls *.md | wc -l | tr -d ' ') files)"
exit $fail

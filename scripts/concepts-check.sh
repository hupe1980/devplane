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
# Nothing a reader of the published product sees may point at these notes.
# `concepts/` and `specs/` are gitignored, so a decision id or a "see the
# architecture notes" in a doc comment renders on docs.rs — and on the site — as
# a reference to a document the reader cannot open.
#
# Two things this guard has been wrong about, both silent: it named a `crates/`
# directory that the single-crate layout removed, so it greped nothing and
# passed; and its own word-boundary filter matched every three-digit id, because
# digits are alphanumeric. Hence the explicit path list, checked for existence.
published="src ui examples tests site README.md CONTRIBUTING.md"
for path in $published; do
  [ -e "../$path" ] || { echo "concepts-check: published path '$path' is missing"; fail=1; }
done
leaked=$(cd .. && grep -rInoE '\b(D|R)[0-9]+\b' $published 2>/dev/null \
  | grep -vE '/target/|site/public/' \
  | grep -vE '[A-Za-z0-9_](D|R)[0-9]|(D|R)[0-9]+[A-Za-z_]')
# The same rule, spelled out rather than numbered: a path into a gitignored
# directory, or a phrase that sends the reader to a document they do not have.
# `tests/documentation.rs` is the one file that may name the notes: it reads
# them to check every `vibeplane.toml` example parses, and skips when they are
# absent — which is the whole reason a clean checkout stays green.
leaked="$leaked$(cd .. && grep -rInoE 'concepts/|specs/|architecture notes|these notes|design notes' \
  $published 2>/dev/null | grep -vE '/target/|site/public/|^tests/documentation\.rs:')"
if [ -n "$leaked" ]; then
  echo "published tree points at these notes (gitignored — the reader has neither):"
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
# trusted.
if command -v cargo >/dev/null 2>&1; then
  ../scripts/deps-count.sh >/dev/null 2>&1 \
    || { echo "the dependency figures in D70/D78 have drifted (scripts/deps-count.sh)"; fail=1; }
fi

# The test count, against a static count of test attributes — which matches what
# `cargo test` reports exactly, so the check costs a grep rather than a build.
# A ledger figure is written `**N tests**`; bold is what separates it from prose
# like "3 tests red", and an unbolded total is therefore invisible here.
tests_in_tree=$(grep -rhE "^[[:space:]]*#\[(tokio::)?test\]" ../src ../tests 2>/dev/null | wc -l | tr -d ' ')
if [ "${tests_in_tree:-0}" -gt 0 ]; then
  claimed=$(grep -rhoE '\*\*[0-9]+ tests\*\*' *.md | grep -oE '[0-9]+' | sort -u | tr '\n' ' ' | sed 's/ $//')
  if [ -n "$claimed" ] && [ "$claimed" != "$tests_in_tree" ]; then
    echo "the test count has drifted: the tree has $tests_in_tree, these notes say $claimed"; fail=1
  fi
fi

# The board page's size, against the page. D167 refuses React on a measurement,
# and the measurement was prose: D16's "270 lines" was wrong by a factor of
# three before anybody noticed, and the figure that replaced it went stale in
# one pass. `tests::ui_contract` holds the *ceiling*; this holds the *claim*.
# Written `N lines and M KB` or `N lines, M KB`, with an optional thin space in
# the thousands, which is how these notes spell numbers.
page="../ui/index.html"
if [ -f "$page" ]; then
  page_lines=$(wc -l < "$page" | tr -d ' ')
  page_kb=$(( ($(wc -c < "$page" | tr -d ' ') + 512) / 1024 ))
  bad=$(grep -rhoE '[0-9][0-9 ]* lines(,| and) [0-9]+ KB' *.md | sort -u | while read -r claim; do
    l=$(echo "$claim" | grep -oE '^[0-9][0-9 ]*' | tr -d ' ')
    k=$(echo "$claim" | grep -oE '[0-9]+ KB' | grep -oE '[0-9]+')
    [ "$l" = "$page_lines" ] && [ "$k" = "$page_kb" ] || echo "$claim"
  done)
  if [ -n "$bad" ]; then
    echo "the page figures have drifted; it is $page_lines lines and $page_kb KB, these notes say:"
    echo "$bad" | sed 's/^/  /'
    fail=1
  fi
fi

# A section reference is a link too, and until now nothing checked it. `§7` in a
# pointer to a file with six sections is a dead end that reads exactly like a
# live one — which is how [ROADMAP.md](ROADMAP.md) §7 survived a pass. The form
# checked is the one these notes actually use: a link to a file, then `§N`.
while read -r line; do
  [ -n "$line" ] || continue
  src=${line%%:*}; rest=${line#*:}; target=${rest%% *}; sec=${rest##*§}
  grep -qE "^## ${sec}\." "$target" || { echo "$src: dangling section reference -> $target §${sec}"; fail=1; }
done <<EOF
$(grep -ohnE '\]\(([A-Z_]+\.md)\) §[0-9]+' *.md >/dev/null 2>&1; \
  for f in *.md; do grep -oE '\]\(([A-Z_]+\.md)\) §[0-9]+' "$f" \
    | sed -E "s/^\]\(//; s/\) §/ §/" | sort -u | sed "s|^|$f:|"; done)
EOF

# ── One number, one home ─────────────────────────────────────────────────────
# Every figure below was found disagreeing with itself across these files in the
# 2026-09-15 pass: the release was 0.1.0 in three documents and 0.2.0 in a
# fourth, the widening count was fourteen in one and eighteen in six. A figure
# repeated in twenty files is a figure that rots in nineteen of them, so each one
# now has a single authority and the rest are checked against it.
#
# The authority is the tree where there is one (Cargo.toml), and ROADMAP.md §2
# otherwise — it is the section a reader is sent to for "what is true today".
# A roadmap item is cited by a stable anchor, never by its position. Five
# references were pointing at the wrong item before this check existed:
# PROVIDERS.md sent a reader to "§3 item 5" three times for review-comment
# handling, and two files to "§4 item 1" for work that has never been in §4. A
# number is a position; positions move and nothing notices.
for a in $(grep -rhoE '`#[a-z][a-z-]+`' *.md | sort -u); do
  grep -qF "$a" ROADMAP.md || { echo "unknown roadmap anchor: $a"; fail=1; }
done
# And the other direction: a numbered item with no anchor cannot be cited safely.
while read -r item; do
  echo "$item" | grep -qE '`#[a-z][a-z-]+`' || { echo "ROADMAP.md: item has no anchor -> ${item:0:60}"; fail=1; }
done < <(grep -E '^\*\*[0-9]+ · ' ROADMAP.md)
# The positional form these anchors replaced, so it cannot come back.
if grep -rnE '§[0-9]+ item [0-9]+' *.md | grep -v '"§'; then
  echo "a roadmap item is cited by position; cite its \`#anchor\` instead"; fail=1
fi

# The released version, against the manifest — the only authority there is.
cargo_version=$(grep -m1 '^version = ' ../Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
if [ -n "$cargo_version" ]; then
  stale=$(grep -lE '\*\*[0-9]+\.[0-9]+\.[0-9]+ (is released|shipped)' *.md 2>/dev/null \
    | while read -r f; do
        grep -oE '\*\*[0-9]+\.[0-9]+\.[0-9]+ (is released|shipped)' "$f" \
          | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | grep -qx "$cargo_version" || echo "$f"
      done)
  [ -z "$stale" ] || { echo "the released version is $cargo_version; these say otherwise: $(echo $stale)"; fail=1; }
fi

# The widening count (R24) and the provider release behaviour was verified
# against. Both are counted rather than compared to a constant, because the
# figure is meant to move; what must not happen is that it moves in one file.
# The count word is matched without its emphasis, since README.md wrote
# `**fourteen times**` and OVERVIEW.md `**eighteen** times` and the two
# disagreed for a whole pass under different markup.
widen=$(grep -rhoiE '(wrong|widened?)[^.]{0,60}?\*{0,2}(four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|twenty-one|twenty-two)(teen)?\*{0,2} times' *.md \
  | grep -oiE '(four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|twenty-one|twenty-two)(teen)? times' \
  | tr 'A-Z' 'a-z' | sort -u)
if [ "$(echo "$widen" | grep -c .)" -gt 1 ]; then
  echo "the widening count (R24) disagrees with itself: $(echo $widen | sed 's/ times//g')"
  grep -rniE '(wrong|widened?)[^.]{0,60}?\*{0,2}[a-z]+\*{0,2} times' *.md | cut -c1-110 | sed 's/^/  /'
  fail=1
fi

# Scoped to lines that actually make the verification claim. A competitor's
# compatibility baseline is also written `Claude Code **2.1.220**` and is not
# this project's figure; the word `verified` is what separates them.
provider=$(grep -rhiE 'verified against[^|]*Claude Code \*\*[0-9]+\.[0-9]+\.[0-9]+\*\*' *.md \
  | grep -oE 'Claude Code \*\*[0-9]+\.[0-9]+\.[0-9]+\*\*' | sort -u)
if [ "$(echo "$provider" | grep -c .)" -gt 1 ]; then
  echo "the provider version verified against disagrees with itself:"
  grep -rniE 'verified against[^|]*Claude Code \*\*[0-9]+\.[0-9]+\.[0-9]+\*\*' *.md | cut -c1-110 | sed 's/^/  /'
  fail=1
fi

[ $fail = 0 ] && echo "concepts-check: ok ($(ls *.md | wc -l | tr -d ' ') files)"
exit $fail

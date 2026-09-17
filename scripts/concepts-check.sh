#!/usr/bin/env bash
# Holds concepts/ to its own rules: every cross-file link resolves, every D/R id is unique,
# every file carries the header block, and no file but ROADMAP.md records unfinished work.
# The three documents that used to be one: DIRECTION.md is the argument, STATE.md is
# the authority for every figure, ROADMAP.md is the backlog (D193).
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
#
# `specs/` is also what a *user's* specification directory is called — Spec Kit
# puts them there — so a documentation example naming `specs/reset.md` is not a
# reference to this repository's gitignored `specs/` at all. The distinguishing
# feature is code: an example lives in a fenced block or a backticked span, and
# a reference to our own notes is bare prose. Matches inside backticks are
# therefore exempt, which keeps the thing this guard is for — "see specs/ for
# the protocol" in a doc comment — and stops it failing an example.
bt=$(printf '\140')
fence="$bt$bt$bt"
notes='concepts/|specs/|architecture notes|these notes|design notes'
examples_excluded=$(cd .. && grep -rInoE "$notes" \
  $published 2>/dev/null | grep -vE '/target/|site/public/|^tests/documentation\.rs:' \
  | while IFS= read -r hit; do
      file=${hit%%:*}; rest=${hit#*:}; line=${rest%%:*}
      text=$(sed -n "${line}p" "$file" 2>/dev/null)
      # Formatted as code on this line: an example, not a reference.
      # A parameter expansion rather than `case`, because a `)` in a case
      # pattern inside a command substitution confuses the parser.
      [ "${text#*$bt}" != "$text" ] && continue
      # Or a string literal, which is what an example path is in Rust. The
      # quoted runs are removed and the line re-tested: if nothing matches any
      # more it was only ever inside a string, and `// see specs/ for the
      # protocol` — the thing this guard is for — still has nothing quoting it.
      stripped=$(printf '%s' "$text" | sed 's/"[^"]*"//g')
      printf '%s' "$stripped" | grep -qE "$notes" || continue
      # Or inside a fenced block, where the line itself carries no backticks.
      opens=$(head -n $((line - 1)) "$file" 2>/dev/null | grep -c "^$fence")
      [ $((opens % 2)) -eq 1 ] && continue
      echo "$hit"
    done)
leaked="$leaked$examples_excluded"
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
# The authority is the tree where there is one (Cargo.toml), and STATE.md
# otherwise — the document a reader is sent to for "what is true today". It was
# ROADMAP.md §2 until the roadmap was split into the argument (DIRECTION.md), the
# figures (STATE.md) and the backlog; a figures table buried inside a backlog is
# read on the wrong errand, and it had nowhere to put a date per row.
# A roadmap item is cited by a stable anchor, never by its position. Five
# references were pointing at the wrong item before this check existed:
# PROVIDERS.md sent a reader to "§3 item 5" three times for review-comment
# handling, and two files to "§4 item 1" for work that has never been in §4. A
# number is a position; positions move and nothing notices.
for a in $(grep -rhoE '`#[a-z][a-z-]+`' *.md | sort -u); do
  grep -qF "$a" ROADMAP.md || { echo "unknown roadmap anchor: $a"; fail=1; }
done
# And the other direction: a roadmap heading with no anchor cannot be cited
# safely. Items used to be numbered (`**7 · Title**`) and are now headings that
# lead with the anchor (`### #work-view · Title`), which is what made the reorder
# in this pass free — the numbers were the only thing a reorder could break.
while read -r item; do
  echo "$item" | grep -qE '^### `?#[a-z][a-z-]+' || { echo "ROADMAP.md: item heading has no anchor -> ${item:0:60}"; fail=1; }
done < <(grep -E '^### ' ROADMAP.md | grep -v '^### M[0-9]')
# The positional form these anchors replaced, so it cannot come back.
if grep -rnE '§[0-9]+ item [0-9]+' *.md | grep -v '"§'; then
  echo "a roadmap item is cited by position; cite its \`#anchor\` instead"; fail=1
fi

# Three versions, and conflating any two of them is how a release goes wrong.
#
#   * the **manifest** version — what a build of this tree produces;
#   * the version these notes **call released** — a claim;
#   * the version the **forge serves** — the world, checked over the network
#     further down.
#
# They are equal between releases, which is why this guard used to compare the
# claim straight to the manifest. They are *not* equal while a release is being
# prepared: the manifest moves first, and for the length of that pass the notes
# still correctly say the older one is what is published. Comparing the two then
# fails a tree that is right.
#
# What is true at every point: the manifest is never **behind** the version the
# notes call released. A build older than the published release is the one state
# that cannot be explained.
cargo_version=$(grep -m1 '^version = ' ../Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
claimed_release=$(grep -hoE '\*\*[0-9]+\.[0-9]+\.[0-9]+ (is released|shipped)' *.md 2>/dev/null \
  | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | sort -Vu | tail -1)
if [ -n "$cargo_version" ] && [ -n "$claimed_release" ]; then
  # Every file has to agree on which version is the released one.
  disagree=$(grep -lE '\*\*[0-9]+\.[0-9]+\.[0-9]+ (is released|shipped)' *.md 2>/dev/null \
    | while read -r f; do
        grep -oE '\*\*[0-9]+\.[0-9]+\.[0-9]+ (is released|shipped)' "$f" \
          | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | grep -qx "$claimed_release" || echo "$f"
      done)
  [ -z "$disagree" ] || { echo "these notes call $claimed_release released; these say otherwise: $(echo $disagree)"; fail=1; }
  # And the manifest may be ahead of it, never behind.
  if [ "$(printf '%s\n%s\n' "$claimed_release" "$cargo_version" | sort -V | tail -1)" != "$cargo_version" ]; then
    echo "the manifest is $cargo_version, behind the $claimed_release these notes call released"
    fail=1
  fi
fi

# The widening count (R24) and the provider release behaviour was verified
# against. Both are counted rather than compared to a constant, because the
# figure is meant to move; what must not happen is that it moves in one file.
# The count word is matched without its emphasis, since README.md wrote
# `**fourteen times**` and OVERVIEW.md `**eighteen** times` and the two
# disagreed for a whole pass under different markup.
widen=$(grep -rhoiE '(wrong|widened?)[^.]{0,60}?\*{0,2}(four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|twenty-(one|two|three|four|five|six|seven|eight|nine)|thirty|thirty-(one|two|three|four|five|six|seven|eight|nine)|forty)(teen)?\*{0,2} times' *.md \
  | grep -oiE '(four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|twenty-(one|two|three|four|five|six|seven|eight|nine)|thirty|thirty-(one|two|three|four|five|six|seven|eight|nine)|forty)(teen)? times' \
  | tr 'A-Z' 'a-z' | sort -u)
# The word list above is finite, and a figure that grows past its end makes this
# check silently stop checking — which is the same failure mode as every other
# thing in here. So the figure has to be *found*, not merely agree with itself.
if [ -z "$widen" ]; then
  echo "the widening count (R24) matched no known number word; extend the list in this script"
  fail=1
fi
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

# ...and against the code, which is the authority for both floors.
#
# Agreeing with itself is not enough for these two. They are the only figures
# here a *user* is shown — `vibeplane doctor` prints them, and the product's
# central claim is how old they are — so the constant the binary reads is the
# fact and these notes are a copy of it, exactly as the released version is a
# copy of `Cargo.toml`. They could drift silently until this existed.
for pair in "VERIFIED_AGAINST verified against" "ROWS_CLEARED_THROUGH rows cleared through"; do
  set -- $pair
  const=$1; shift
  phrase="$*"
  in_code=$(grep -oE "pub const $const: &str = \"[0-9]+\.[0-9]+\.[0-9]+\"" ../src/core/policy.rs \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
  [ -n "$in_code" ] || continue
  # `.{0,40}` rather than `[^|]*`: the figure's own row in STATE.md is a table
  # cell, so the phrase and the version sit either side of a `|` and a pattern
  # that refuses pipes reads every occurrence *except* the authoritative one.
  # That is how the first version of this check passed a deliberately wrong
  # figure — it was matching prose elsewhere and never the row it is about.
  in_notes=$(grep -rhoiE "$phrase.{0,40}Claude Code \*\*[0-9]+\.[0-9]+\.[0-9]+\*\*" *.md \
    | grep -oE 'Claude Code \*\*[0-9]+\.[0-9]+\.[0-9]+' \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | sort -u)
  [ -n "$in_notes" ] || continue
  if [ "$in_notes" != "$in_code" ]; then
    echo "policy::$const is $in_code; these notes say $(echo $in_notes) for '$phrase'"
    fail=1
  fi
done

# ── Facts about somebody else's server ───────────────────────────────────────
# Every guard above is a guard over *this* repository: the test count against the
# tree, the page against the page, the released version against `Cargo.toml`, the
# widening count and the anchors against each other. That is why the two wrong
# figures in the 2026-09-16 pass were both external and neither failed anything —
# `v0.3.0` was published while STATE.md's ancestor said the tag was unpushed (the guard
# compared the claim to the manifest, which agreed, because the claim was about
# GitHub), and Claude Code 2.1.273 had shipped while §2 called 2.1.272 "the
# current release" (nothing was the authority for that at all).
#
# Three requests close them. They are skipped without a word when the network is
# unavailable, so a clean offline checkout stays green; `VIBEPLANE_NO_NET=1`
# skips them on purpose.
#
# The third was added after the first two had already been written *from a
# post-mortem*: they fixed the two facts that had just been wrong and not the
# class, and the class is "any figure about somebody else's machine". The next
# member showed up one pass later and by hand — the conformance suite pins
# `@github/copilot@1.0.83` while npm publishes 1.0.85, and a note said the pin
# "is still the published version". Nothing failed, and nothing could.
#
# What is still refused, and the reason has not changed: a link-checker over
# REFERENCES.md and a star-count refresher. A guard that is slow or flaky is a
# guard people start skipping, and a skipped guard is how the internal figures
# rotted before any of this existed. Those stay a person's job once per pass.
if [ -z "${VIBEPLANE_NO_NET:-}" ] && command -v curl >/dev/null 2>&1; then
  get() { curl -fsS --max-time 8 "$1" 2>/dev/null; }

  # 1. The release these notes call released, against what the forge serves.
  #    `releases/latest` redirects to the tag, which is the one answer that
  #    reflects a *published* release rather than a pushed tag: a tag with no
  #    release behind it leaves the installer serving the release before it.
  #    The **claim** is what is compared, never the manifest: during a version
  #    bump the manifest is deliberately ahead of what is published, and a guard
  #    that compared it would fail every release preparation.
  if [ -n "${claimed_release:-}" ]; then
    latest=$(curl -fsS -o /dev/null -w '%{redirect_url}' --max-time 8 \
      https://github.com/hupe1980/vibeplane/releases/latest 2>/dev/null | sed 's|.*/tag/||')
    if [ -n "$latest" ] && [ "$latest" != "v$claimed_release" ]; then
      echo "these notes call $claimed_release released; the forge serves $latest"
      fail=1
    fi
  fi

  # 2. The changelog floor, against the vendor's changelog head. This is the
  #    figure `#harness-clock` acts on: a release above the floor is a run owed,
  #    and the gap between the floor and the head is where a widening — or, in
  #    2.1.273, a revert that made this matcher stricter than the product — lives
  #    unseen. One home for the figure, STATE.md, same as every other.
  read_through=$(grep -E '^\| Changelog rows cleared through \|' STATE.md 2>/dev/null \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
  if [ -n "$read_through" ]; then
    head_ver=$(get https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md \
      | grep -m1 -oE '^## [0-9]+\.[0-9]+\.[0-9]+' | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
    if [ -n "$head_ver" ] && [ "$head_ver" != "$read_through" ]; then
      echo "the changelog floor is $read_through; Claude Code is at $head_ver — those rows are unread"
      echo "  (scripts/changelog-rows.sh, then policy::ROWS_CLEARED_THROUGH and STATE.md)"
      fail=1
    fi
  fi

  # 3. What these notes say a registry publishes, against what it publishes.
  #
  #    **The pin itself does not move**: nothing here auto-updates, and a
  #    conformance suite's value is that it ran against a known version. What
  #    goes stale is the sentence beside it — `#copilot` said the pin "is still
  #    the published version" while npm had moved on twice, in exactly the blind
  #    spot the two checks above were written to close and did not generalise
  #    out of.
  #
  #    So this is a *figure* check like every other one here, not a phrase
  #    heuristic: the first attempt grepped for "still the published version"
  #    and fired on two files that were *quoting* the stale claim in order to
  #    retire it. One home for the number — STATE.md — and the rest is
  #    comparison.
  #    The pattern was wrong on its first outing, in the way every guard here
  #    has been wrong at least once: it assumed the bold wrapped the *number*
  #    (`npm publishes **1.0.85**`) where the file bolds the whole phrase, so it
  #    matched nothing and passed. A guard that reads as installed and checks
  #    nothing is the failure this whole file exists to make loud, and it is
  #    why the check below was run against a deliberately wrong figure before
  #    being believed.
  published_claim=$(grep -oE 'npm publishes [0-9]+\.[0-9]+\.[0-9]+' STATE.md 2>/dev/null \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
  if [ -n "$published_claim" ]; then
    published=$(get https://registry.npmjs.org/@github/copilot/latest \
      | grep -oE '"version":"[0-9]+\.[0-9]+\.[0-9]+"' | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
    if [ -n "$published" ] && [ "$published" != "$published_claim" ]; then
      echo "these notes say npm publishes @github/copilot $published_claim; it publishes $published"
      fail=1
    fi
  fi
fi

[ $fail = 0 ] && echo "concepts-check: ok ($(ls *.md | wc -l | tr -d ' ') files)"
exit $fail

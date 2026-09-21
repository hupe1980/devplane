#!/usr/bin/env bash
# Holds concepts/ to its own rules: every cross-file link resolves, every D/R id is unique,
# every file carries the header block, and no file but ROADMAP.md records unfinished work.
# The three documents that used to be one: DIRECTION.md is the argument, STATE.md is
# the authority for every figure, ROADMAP.md is the backlog (D193).
# A recipe rather than a CI stage because concepts/ is untracked.
set -u
cd "$(dirname "$0")/../concepts" || { echo "no concepts/ directory"; exit 1; }
fail=0

# ---- the vacuity rule, applied to this file instead of to one guard --------
#
# **A guard that finds nothing to compare must fail** (D309). That rule was
# bought on 2026-09-20 by the line-count guard, written into QUALITY.md §4, and
# then applied to exactly the one guard that bought it. On 2026-09-21 a sweep of
# every extraction in this file found **three more that had never matched
# anything** — including the release guard, whose own row in STATE.md cites it as
# the model of a guard that works ("found stale within minutes of the tag going
# up"). It had been inert since the row it reads was re-spelled, and while it was
# inert the notes went on calling 0.5.0 the published release after v0.6.0 shipped.
#
# So the rule stops being advice and becomes a function. Any extraction that
# feeds a comparison is wrapped in `expect`, and an empty one is a failure with
# the guard's name on it — because *absent* and *correct* are different answers
# and only one of them is what "every figure has one home" means.
expect() { # expect <guard name> <extracted value>
  [ -n "$2" ] && return 0
  echo "guard '$1' matched nothing: the claim it reads is absent or has been re-spelled"
  fail=1
  return 1
}
for f in *.md; do
  # links
  grep -o '\]([A-Z_]*\.md\(#[a-z0-9-]*\)\?)' "$f" | sed 's/^](//; s/)$//; s/#.*//' | sort -u | while read -r t; do
    [ -f "$t" ] || { echo "$f: broken link -> $t"; exit 9; }
  done || fail=1
  # header
  [ "$f" = README.md ] || grep -q '^> Part of the Devplane architecture notes' "$f" || { echo "$f: missing header block"; fail=1; }
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
# `concepts/` and `reference/` are gitignored, so a decision id or a "see the
# architecture notes" in a doc comment renders on docs.rs — and on the site — as
# a reference to a document the reader cannot open.
#
# Two things this guard has been wrong about, both silent: it named a `crates/`
# directory that the single-crate layout removed, so it greped nothing and
# passed; and its own word-boundary filter matched every three-digit id, because
# digits are alphanumeric. Hence the explicit path list, checked for existence.
# `ui/` now carries a node project, so every scan over the published tree skips
# `node_modules/` (somebody else's code, which cites its own identifiers) and
# `ui/dist/` (generated, and a copy of what is already checked at source).
# Without that, `lib.dom.d.ts` reports an `R8` and this guard fails on
# TypeScript's own type definitions.
published="src ui examples tests site README.md CONTRIBUTING.md"
for path in $published; do
  [ -e "../$path" ] || { echo "concepts-check: published path '$path' is missing"; fail=1; }
done
leaked=$(cd .. && grep -rInoE '\b(D|R)[0-9]+\b' $published 2>/dev/null \
  | grep -vE '/target/|site/public/|/node_modules/|^ui/dist/' \
  | grep -vE '[A-Za-z0-9_](D|R)[0-9]|(D|R)[0-9]+[A-Za-z_]')
# The same rule, spelled out rather than numbered: a path into a gitignored
# directory, or a phrase that sends the reader to a document they do not have.
# `tests/documentation.rs` is the one file that may name the notes: it reads
# them to check every `devplane.toml` example parses, and skips when they are
# absent — which is the whole reason a clean checkout stays green.
#
# `reference/` is also what a *user's* specification directory is called — Spec Kit
# puts them there — so a documentation example naming `reference/reset.md` is not a
# reference to this repository's own corpus at all. That corpus now lives at
# `concepts/reference/` and is covered by the `concepts/` half of this pattern; the
# bare `reference/` half is kept because the thing it catches is prose — "see
# reference/ for the protocol" in a doc comment — which names nothing a reader of
# the published tree can open, before the move or after it. The distinguishing
# feature is code: an example lives in a fenced block or a backticked span, and
# a reference to our own notes is bare prose. Matches inside backticks are
# therefore exempt, which keeps the thing this guard is for — "see reference/ for
# the protocol" in a doc comment — and stops it failing an example.
bt=$(printf '\140')
fence="$bt$bt$bt"
notes='concepts/|reference/|architecture notes|these notes|design notes'
examples_excluded=$(cd .. && grep -rInoE "$notes" \
  $published 2>/dev/null | grep -vE '/target/|site/public/|/node_modules/|^ui/dist/|^tests/documentation\.rs:' \
  | while IFS= read -r hit; do
      file=${hit%%:*}; rest=${hit#*:}; line=${rest%%:*}
      text=$(sed -n "${line}p" "$file" 2>/dev/null)
      # Formatted as code on this line: an example, not a reference.
      # A parameter expansion rather than `case`, because a `)` in a case
      # pattern inside a command substitution confuses the parser.
      [ "${text#*$bt}" != "$text" ] && continue
      # Or a string literal, which is what an example path is in Rust. The
      # quoted runs are removed and the line re-tested: if nothing matches any
      # more it was only ever inside a string, and `// see reference/ for the
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

# A specification's identifiers are the same failure as a `(D9)`, one layer over:
# `SC-007` and `T018` mean nothing to somebody without the folder that defines
# them. Comment lines only, and backticked runs are exempt — `FR-001` inside a
# sentence *about* the format Spec Kit writes is a fact about the world, and the
# test fixtures that contain `- [ ] T001` are a user's specification, not ours.
ids=$(cd .. && grep -rInE '^[[:space:]]*(//|///|//!)' $published 2>/dev/null \
  | grep -vE '/target/|site/public/|/node_modules/|^ui/dist/' \
  | while IFS= read -r hit; do
      text=${hit#*:}; text=${text#*:}
      printf '%s' "$text" | sed "s/$bt[^$bt]*$bt//g" \
        | grep -qE '\b(FR|SC)-[0-9]{3}\b|\bT[0-9]{3}\b' && echo "$hit"
    done)
if [ -n "$ids" ]; then
  echo "published tree cites a specification's identifiers (gitignored):"
  echo "$ids" | sed 's/^/  /'
  fail=1
fi

# The same rule for this project's own specifications, and it needs a different
# mechanism. `specs/NNN-name/` is also where a **user's** specifications live —
# it is what `--spec` points at and it is documented as a product feature — so
# the path alone says nothing about whose it is, and the backtick heuristic above
# exempts exactly the citations worth catching (`Specified in
# `specs/001-work-view-diff`` reads as an example and is not one).
#
# So this asks the only question with an exact answer: do any of *this
# checkout's* feature directories appear by name in the published tree? A clean
# clone has no `specs/` and nothing to check, which is correct rather than
# lenient — there is nothing there to point at.
if [ -d ../specs ]; then
  for feature in ../specs/*/; do
    [ -d "$feature" ] || continue
    name=$(basename "$feature")
    hits=$(cd .. && grep -rIn --fixed-strings "$name" $published 2>/dev/null \
      | grep -vE '/target/|site/public/|/node_modules/|^ui/dist/')
    if [ -n "$hits" ]; then
      echo "published tree names a gitignored specification ($name):"
      echo "$hits" | sed 's/^/  /'
      fail=1
    fi
  done
fi
# A specification and the roadmap item it serves must agree about whether the
# work is done, and until 2026-09-19 nothing compared them. ROADMAP.md calls
# itself "the only file in these notes that may record unfinished work" while
# `specs/*/tasks.md` held 106 checkboxes exempt from that rule by living outside
# concepts/ — and the two disagreed for two passes: the roadmap said "neither is
# built" while the task lists said 31/31 and 29/29 and the code agreed with the
# task lists (D262).
#
# Two rules, both cheap:
#   1. every specification folder is named by exactly one roadmap item, so a
#      specification nobody is working from is visible;
#   2. a specification whose tasks are ALL complete is not named at all, because
#      it has shipped and the convention is that it is deleted (003-007 are gone).
if [ -d ../specs ]; then
  for feature in ../specs/*/; do
    [ -d "$feature" ] || continue
    name=$(basename "$feature")
    # `grep -c` prints 0 AND exits 1 when nothing matches, so `|| echo 0` yields
    # "0\n0" and every downstream `[` throws "integer expression expected" — which
    # bash reports to stderr while the check goes on to pass. Caught by watching
    # this guard fail, which is the only reason it is not still here.
    mentions=$(grep -c -- "$name" ROADMAP.md 2>/dev/null || true)
    mentions=${mentions:-0}
    tasks="$feature/tasks.md"
    if [ -f "$tasks" ]; then
      open_tasks=$(grep -c '^- \[ \]' "$tasks" 2>/dev/null || true)
      open_tasks=${open_tasks:-0}
    else
      open_tasks=1
    fi
    if [ "$open_tasks" -eq 0 ]; then
      [ "$mentions" -eq 0 ] || {
        echo "specs/$name has no open tasks but ROADMAP.md still names it — it shipped; delete the folder (D262)"
        fail=1
      }
    else
      [ "$mentions" -ge 1 ] || {
        echo "specs/$name has $open_tasks open task(s) and no roadmap item names it (D262)"
        fail=1
      }
    fi
  done
fi

# And the machinery beside them. Unlike a feature name this string is fixed, so
# it is checked whether or not the directory is here.
#
# **`extensions.yml` is exempt, and the exemption is the point rather than a
# hole in the check.** This guard exists so the published tree never points a
# reader at a path that is not in the repository. That reasoning held while
# `.specify/` meant only this project's own gitignored working copy. It stopped
# holding when `devplane speckit install` shipped: that file is now a documented
# interface in **the user's** repository, and a reference to it is as ordinary
# as one to `devplane.toml`. Suppressing it would have meant documenting a
# command without naming the file it writes.
#
# Everything else under `.specify/` — the templates, the memory, the scripts —
# is still this project's scratch, and still fails here.
specify_hits=$(cd .. && grep -rIn --fixed-strings '.specify/' $published 2>/dev/null \
  | grep -vE '/target/|site/public/|/node_modules/|^ui/dist/' \
  | grep -v '\.specify/extensions\.yml')
if [ -n "$specify_hits" ]; then
  echo "published tree points into .specify/ (gitignored):"
  echo "$specify_hits" | sed 's/^/  /'
  fail=1
fi

# Every `devplane.toml` example has to be one the parser accepts. `deny_unknown_fields`
# means a single designed-but-unbuilt key fails the whole file — and takes that repository's
# permission rules down with it — so a reference that cannot be pasted is worse than none.
# It has happened twice: five sections that did not exist, and an `on =` key for standing
# pipelines. `tests/documentation.rs` checks both these notes and the README; it skips the
# notes when they are absent, which is why it is worth running from here as well.
if command -v cargo >/dev/null 2>&1; then
  ( cd .. && cargo test --quiet --test documentation ) >/dev/null 2>&1 \
    || { echo "a devplane.toml example does not parse (cargo test --test documentation)"; fail=1; }
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

# The line count, on the same rule and for the same reason (D292). The test
# count has had a guard since the first pass and was right in four places; this
# figure had none and was wrong in two on the day after it was written — inside
# a row whose whole purpose was to stop a figure living in four files.
#
# `src/` only, because that is the figure the notes quote and because a number
# covering tests moves for reasons nobody means by "how big is this". As with
# the test count, **bold** is what makes a figure a claim: "42,826 lines of
# Rust" in a sentence about 2026-09-19 is history and is deliberately invisible
# here, while `**49,603 lines**` is an assertion about today.
#
# **And this guard passed vacuously for the whole of its life**, which is the
# defect it now also checks for. It looked for `**N lines**` and STATE.md's row
# is written `| Lines of Rust | **53,280** in \`src/\`` — the bold ends before
# the word. So `claimed_lines` was empty, the comparison never ran, the script
# said `ok`, and the figure it was added for (D292) drifted by 556 lines
# unseen. A guard that finds nothing to compare must say so rather than pass:
# an absent claim and a correct one are not the same outcome, and only one of
# them is what "every figure has one home" means.
src_lines=$(find ../src -name '*.rs' -exec cat {} + 2>/dev/null | wc -l | tr -d ' ')
if [ "${src_lines:-0}" -gt 0 ]; then
  # Two spellings, because the figure has one home and that home writes it as a
  # table cell: `**N lines**` anywhere, and the first bold figure in the row
  # labelled `Lines of Rust`. The notes write thousands with a comma and `wc`
  # does not, so the comparison is made on the digits and the message quotes the
  # claim as it is written.
  #
  # The authority row is read for its **first** bold figure and no other. The
  # row explains the defect this guard was fixed for and therefore quotes the
  # old spelling — `**53,280**` — in its own third column, so a guard reading
  # every bold figure on the row fails on the history of its own repair. That
  # is the fifth pass's lesson exactly: match the cell's mark, not its prose.
  # And the copies are scoped to figures that are about *Rust*. The loose form
  # — any `**N lines**` anywhere — matched a sentence in PASSES.md about how
  # long ROADMAP.md was, which is a true figure about a markdown file and has
  # nothing to do with `src/`. A guard that fails on an unrelated correct number
  # is a guard somebody switches off.
  claimed_lines=$( { grep -rh '^| Lines of Rust |' *.md \
                       | grep -oE '\*\*[0-9][0-9,]*( lines)?\*\*' | head -1
                     grep -rhE '(src/|lines of Rust)' *.md \
                       | grep -oE '\*\*[0-9][0-9,]* lines\*\*'
                   } | grep -oE '[0-9][0-9,]*' | sort -u | tr '\n' ' ' | sed 's/ $//')
  claimed_digits=$(echo "$claimed_lines" | tr -d ',')
  if [ -z "$claimed_lines" ]; then
    echo "the line count has no guarded home: src/ has $src_lines and no file states it as a checkable figure"; fail=1
  elif [ "$claimed_digits" != "$src_lines" ]; then
    echo "the line count has drifted: src/ has $src_lines, these notes say $claimed_lines"; fail=1
  fi
fi

# Every attention kind in the code is documented, and every kind documented as
# built exists in the code.
#
# This guard exists because ATTENTION.md's header said "eighteen kinds", D250
# said "twenty-two item kinds", and the enum had twenty-one — three numbers for
# one set, none of them checked. Counting was the wrong instrument anyway: the
# table spells two kinds on one row, so a count disagreed with the truth while
# the *set* matched exactly.
#
# So this compares sets, not totals, in both directions. An enum variant nobody
# documented is a surface with no description; a kind documented as ✅ that the
# enum does not have is the "documented trigger with no code" rule failing in
# the file that states it.
if [ -f ../src/core/attention.rs ]; then
  code_kinds=$(sed -n '/pub enum AttentionKind/,/^}/p' ../src/core/attention.rs \
    | grep -oE '^    [A-Z][A-Za-z]+' | tr -d ' ' \
    | sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr 'A-Z' 'a-z' | sort -u)
  # A row may carry several kinds: "| ✅ `issue_assigned` · `review_requested` |".
  # ⏳ counts as present-in-code too: it means built and incomplete, not absent.
  doc_kinds=$(grep -oE '^\| (✅|⏳) (`[a-z_]+`( · )?)+' ATTENTION.md \
    | grep -oE '`[a-z_]+`' | tr -d '`' | sort -u)
  missing_doc=$(comm -23 <(echo "$code_kinds") <(echo "$doc_kinds"))
  missing_code=$(comm -13 <(echo "$code_kinds") <(echo "$doc_kinds"))
  [ -z "$missing_doc" ] || {
    echo "attention kinds in the code and not in ATTENTION.md: $(echo $missing_doc)"; fail=1; }
  [ -z "$missing_code" ] || {
    echo "attention kinds ATTENTION.md calls built that the code lacks: $(echo $missing_code)"; fail=1; }
fi

# The claim ledger's size, against the script that produces it. This guard exists
# because the figure it checks was wrong by twenty-six for a pass: STATE.md said
# `**209/209**` while `verify-claims.sh` was checking 235, and QUALITY.md's own
# header said `179/179` — three spellings of one number, in the two files that
# state the rule that there may only be one. The test count has had a guard for
# passes and this, beside it, had none.
#
# Counted from the `chk` lines rather than by running the script: the ledger needs
# a fetched corpus and this check must stay green in a clean checkout. The two
# numbers therefore agree by construction unless somebody writes a `chk` the shell
# would not run, which is the same assumption the test-count guard makes.
# The claim-ledger figure, against the script that produces it — by running it,
# not by counting its source. A static count of `chk` lines was tried first and
# was wrong twice in five minutes: 235, then 227, against the script's own 238.
# The counter is incremented at *run* time, inside three differently-named
# helpers, some of whose calls sit in conditionals and loops, so the only honest
# count is the one the script prints. That is the same rule this repository
# applies to everything else — a document is not the product — turned on itself.
#
# Skipped without comment when the corpus is absent, exactly like the network
# checks below, so a clean checkout stays green.
if [ -d reference ]; then
  ledger_line=$(sh ../scripts/verify-claims.sh 2>/dev/null | grep -oE '[0-9]+ claims checked')
  claims_in_script=${ledger_line%% *}
  if [ -n "${claims_in_script:-}" ] && [ "${claims_in_script:-0}" -gt 0 ]; then
    # Scoped to lines that say `ledger`, because `**N/M**` is also how these notes
    # spell "3 of 75 papers" — REFERENCES.md carries one, and an unscoped pattern
    # reported it as a ledger figure that was not N/N. A guard whose first run
    # produces a false positive teaches the next reader to skip its output.
    claimed=$(grep -rhiE '.*ledger.*' *.md | grep -ohE '\*\*[0-9]+/[0-9]+\*\*' | sort -u | tr '\n' ' ' | sed 's/ $//')
    for c in $claimed; do
      n=$(echo "$c" | sed 's/\*//g; s|/.*||')
      d=$(echo "$c" | sed 's/\*//g; s|.*/||')
      [ "$n" = "$d" ] || { echo "a ledger figure is not N/N: $c"; fail=1; }
      [ "$n" = "$claims_in_script" ] || {
        echo "the claim-ledger figure has drifted: verify-claims.sh checks $claims_in_script, these notes say $c"; fail=1; }
    done
  fi
fi

# The decision and risk counts, against the tables that define them. `DECISIONS.md`
# and `RISKS.md` are the homes; a prose figure elsewhere ("252 decisions") is a
# copy, and every copy in this repository has rotted at least once.
d_defined=$(grep -c '^| D[0-9]\+ ' DECISIONS.md 2>/dev/null || echo 0)
r_defined=$(grep -c '^| R[0-9]\+ ' RISKS.md 2>/dev/null || echo 0)
for pair in "$d_defined decisions" "$r_defined risks"; do
  set -- $pair
  [ "$1" -gt 0 ] || continue
  claimed=$(grep -rhoE "\*\*[0-9]+ $2\*\*" *.md | grep -oE '[0-9]+' | sort -u | tr '\n' ' ' | sed 's/ $//')
  if [ -n "$claimed" ] && [ "$claimed" != "$1" ]; then
    echo "the $2 count has drifted: the table has $1, these notes say $claimed"; fail=1
  fi
done

# The board page's size, against the page. D167 refuses React on a measurement,
# and the measurement was prose: D16's "270 lines" was wrong by a factor of
# three before anybody noticed, and the figure that replaced it went stale in
# one pass. `tests::ui_contract` holds the *ceiling*; this holds the *claim*.
# Written `N lines and M KB` or `N lines, M KB`, with an optional thin space in
# the thousands, which is how these notes spell numbers.
#
# **And in the other order, which is the hole this guard had.** It matched lines
# before KB only, so `96 KB and 2 091 lines` in [UX.md](UX.md) Â§7 sat two
# figures stale through every pass that ran this script green â in the
# document that defines the page-weight budget. A guard that reads one spelling
# of a figure is a guard that teaches the next writer the other spelling.
# The **legacy** page, deliberately, and only until the switch. `ui/index.html`
# is now the built interface's entry — seventeen lines that say nothing about
# the served size — and pointing this at it turned a guard over the thing being
# replaced into a guard over a stub. The figure it protects is the one the
# rebuild is measured against, so it follows the artefact rather than the name.
page="../ui/legacy.html"
if [ -f "$page" ]; then
  page_lines=$(wc -l < "$page" | tr -d ' ')
  page_kb=$(( ($(wc -c < "$page" | tr -d ' ') + 512) / 1024 ))
  bad=$( { grep -rhoE '[0-9][0-9 ]* lines(,| and) [0-9]+ KB' *.md
           grep -rhoE '[0-9]+ KB(,| and) [0-9][0-9 ]* lines' *.md
         } | sort -u | while read -r claim; do
    l=$(echo "$claim" | grep -oE '[0-9][0-9 ]* lines' | grep -oE '^[0-9][0-9 ]*' | tr -d ' ')
    k=$(echo "$claim" | grep -oE '[0-9]+ KB' | grep -oE '[0-9]+')
    [ "$l" = "$page_lines" ] && [ "$k" = "$page_kb" ] || echo "$claim"
  done)
  # **And a bare line count, with no KB beside it.** The pair guard above reads
  # two spellings of *lines and KB together*; `a hand-rolled renderer at 2435
  # lines` matched neither, so one decision row carried two different counts for
  # one file — the constitution's "one figure, one home" broken inside a single
  # sentence. Any four-digit "NNNN lines" in these notes is about this page.
  bare=$(grep -rhoE '\b[0-9]{4} lines\b' *.md | sort -u | grep -v "^$page_lines lines$" || true)
  [ -n "$bare" ] && bad="$bad
$bare"
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

# And the same check for a sub-section, which the pattern above could not see
# because it stops at the first `.` — so `§2.1` was read as `§2`, matched a
# heading about something else, and passed. Twelve such references were live on
# 2026-09-20: `MARKET_LANDSCAPE.md` §2.1, §3.1, §3.2, §3.3 and §4.1 all named
# content that had been rewritten out of the file, and four of them pointed at a
# second persona the file no longer described at all. A reference that resolves
# to the wrong thing and one that resolves to nothing fail the same way — the
# reader follows it and finds a stranger (D291).
#
# Sub-sections are written `### 2.1 Title` or `#### 5.1.1 Title` — no dot after
# the number, unlike the top-level `## 2. Title` — so both forms are accepted
# here rather than assuming one.
while read -r line; do
  [ -n "$line" ] || continue
  src=${line%%:*}; rest=${line#*:}; target=${rest%% *}; sec=${rest##*§}
  grep -qE "^#{2,4} ${sec}[. ]" "$target" \
    || { echo "$src: dangling sub-section reference -> $target §${sec}"; fail=1; }
done <<EOF
$(for f in *.md; do grep -oE '\]\(([A-Z_]+\.md)\) §[0-9]+\.[0-9]+[a-z]?' "$f" \
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
#
# **It must match a heading, not merely the file.** The first form of this check
# looked for the anchor anywhere in ROADMAP.md, so an item that was finished and
# deleted stayed "known" for as long as its own prose still mentioned it
# elsewhere in the file — which is what happened the first time an item was
# retired, and the check was green for it.
#
# A CSS hex colour is also a backticked `#` followed by letters — `#ffffff` and
# `#fff` match this pattern exactly — and DESIGN.md is full of them. They are
# excluded by shape rather than by filename, because the exclusion has to keep
# working when the tokens move to another file: a three- or six-character run of
# hex digits is never an anchor, and an anchor is never only hex digits.
for a in $(grep -rhoE '`#[a-z][a-z-]+`' *.md | grep -vE '^`#([0-9a-f]{3}|[0-9a-f]{6})`$' | sort -u); do
  grep -qE "^### $a " ROADMAP.md || { echo "unknown roadmap anchor: $a"; fail=1; }
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

# ROADMAP.md's own scope line says it records unfinished work and nothing else,
# and for six passes it recorded mostly finished work: five items headed
# "Shipped" with their post-mortems attached, at about 350 lines. Three separate
# passes wrote down that "no check in this repository compares a roadmap item to
# the tree" and none of them added one.
#
# This is the honest form of that check. It cannot tell whether an *unbuilt*
# item is secretly done — that needs a reading, and the reading is what found
# `#vacuity` and `#work-view`. What it can do is enforce the rule that was
# actually broken: **a finished item does not live in the backlog.** Its
# reasoning goes to PASSES.md, which is the file for what a pass bought, and its
# anchor goes to the Retired section so the citations from other documents still
# resolve.
#
# The Retired section is quarantined rather than exempted: everything under it is
# deleted work kept for its anchor, which is the one place a past tense belongs.
retired_line=$(grep -nE '^## .*Retired' ROADMAP.md | head -1 | cut -d: -f1)
: "${retired_line:=999999}"
while IFS=: read -r ln item; do
  [ "$ln" -ge "$retired_line" ] && continue
  case "$item" in
    *Shipped*|*shipped\ 2026-*|*✅*|*Settled\ 2026-*)
      echo "ROADMAP.md:$ln: the backlog records finished work -> ${item:0:70}"
      echo "  move what it bought to PASSES.md and its anchor to the Retired section"
      fail=1 ;;
  esac
done < <(grep -nE '^### ' ROADMAP.md | grep -v '^[0-9]*:### M[0-9]')

# And every item states the two things that make a backlog rankable. An item
# with no size cannot be ordered against one that has one, and an item with no
# exit condition is a wish: both were true of rows that sat near the top of this
# list for six passes. The size is on the heading or the line under it; the exit
# condition is the file's own "*Settled when:*" form.
awk -v retired="$retired_line" '
  /^### / && NR < retired {
    if (anchor != "" && !(sized && exited)) {
      printf "ROADMAP.md:%d: item %s is missing %s%s%s\n", start, anchor,
        (sized ? "" : "a size"), ((!sized && !exited) ? " and " : ""), (exited ? "" : "an exit condition")
      bad = 1
    }
    anchor = $2; start = NR; sized = 0; exited = 0; next
  }
  /^## / && NR < retired {
    if (anchor != "" && !(sized && exited)) {
      printf "ROADMAP.md:%d: item %s is missing %s%s%s\n", start, anchor,
        (sized ? "" : "a size"), ((!sized && !exited) ? " and " : ""), (exited ? "" : "an exit condition")
      bad = 1
    }
    anchor = ""; next
  }
  anchor != "" && /\*\((hours|an afternoon|days|a week|weeks|~?[0-9]+ ?(days|weeks))\)?/ { sized = 1 }
  anchor != "" && /\*Settled when/ { exited = 1 }
  END {
    if (anchor != "" && !(sized && exited)) {
      printf "ROADMAP.md:%d: item %s is missing %s%s%s\n", start, anchor,
        (sized ? "" : "a size"), ((!sized && !exited) ? " and " : ""), (exited ? "" : "an exit condition")
      bad = 1
    }
    exit bad
  }' ROADMAP.md || fail=1

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
#
# **The pattern reads the row, not a turn of phrase.** It used to look for
# `**N.N.N is released**` / `**N.N.N shipped**` — a spelling that appears nowhere
# in these notes — so it matched nothing and skipped both halves of this check
# for the life of the project. STATE.md §2's Release row is the one home for this
# figure, so the guard reads that row's own shape and `expect` makes an absent
# row as loud as a wrong one.
claimed_release=$(grep -oE '^\| Release \| \*\*[0-9]+\.[0-9]+\.[0-9]+\*\*' STATE.md 2>/dev/null \
  | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
expect "the released version (STATE.md \`| Release |\` row)" "$claimed_release" || :
if [ -n "$cargo_version" ] && [ -n "$claimed_release" ]; then
  # Every other file has to agree with that row, in whatever words it uses.
  #
  #    **PASSES.md is exempt, and the exemption is the register rather than a
  #    convenience.** It is the pass log: it records what was true on a day, so
  #    "the manifest led the released 0.5.0" is a correct sentence about
  #    2026-09-20 and must stay wrong-looking for ever. The first run of this
  #    re-pointed guard fired on exactly that line — the same shape as the npm
  #    guard firing on files that were *quoting* a stale claim in order to
  #    retire it — which is why a guard is run before it is believed.
  disagree=$(grep -lE 'released [0-9]+\.[0-9]+\.[0-9]+|[0-9]+\.[0-9]+\.[0-9]+ is (the )?(current|published) release' *.md 2>/dev/null \
    | grep -v '^PASSES\.md$' \
    | while read -r f; do
        grep -ohE 'released [0-9]+\.[0-9]+\.[0-9]+|[0-9]+\.[0-9]+\.[0-9]+ is (the )?(current|published) release' "$f" \
          | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | grep -qx "$claimed_release" || echo "$f"
      done)
  [ -z "$disagree" ] || { echo "these notes call $claimed_release released; these say otherwise: $(echo $disagree)"; fail=1; }
  # And the manifest may be ahead of it, never behind.
  if [ "$(printf '%s\n%s\n' "$claimed_release" "$cargo_version" | sort -V | tail -1)" != "$cargo_version" ]; then
    echo "the manifest is $cargo_version, behind the $claimed_release these notes call released"
    fail=1
  fi
fi

# The site states the version too, in structured data on every page, and it is
# the one copy no guard reached: it said 0.1.0 for four releases. It is the same
# claim as the manifest's, so it is checked against it rather than trusted.
site_version=$(grep -m1 '^version = ' ../site/zola.toml 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
if [ -n "$site_version" ] && [ -n "$cargo_version" ] && [ "$site_version" != "$cargo_version" ]; then
  echo "site/zola.toml says $site_version; the manifest says $cargo_version"
  fail=1
fi


# The schema version, which is a figure about this tree and drifted unnoticed.
#
# STATE.md said `version 1` while `store::SCHEMA_VERSION` said 2 — the `looks`
# table bumped the constant and nobody re-read the row. It is exactly the class
# every other guard here exists for and it had no guard, because the figure is
# a word (`version N`) rather than a bold number.
schema_code=$(grep -oE 'pub const SCHEMA_VERSION: i64 = [0-9]+' ../src/store.rs \
  | grep -oE '[0-9]+$')
expect "store::SCHEMA_VERSION" "$schema_code" || :
schema_notes=$(grep -oE '^\| Schema \| \*\*version [0-9]+\*\*' STATE.md | grep -oE '[0-9]+')
expect "the schema version in STATE.md" "$schema_notes" || :
if [ -n "$schema_code" ] && [ -n "$schema_notes" ] && [ "$schema_code" != "$schema_notes" ]; then
  echo "the schema version has drifted: the code says $schema_code, these notes say $schema_notes"
  fail=1
fi

# The pass ordinal, which is a figure like any other and had no home until
# 2026-09-21. PASSES.md's header says which pass is newest; the file's own
# headings are the count. They disagreed for three passes — four documents cite
# "the eleventh pass" while this file held thirteen — and a pass then named
# itself the twelfth in four files before anybody read the headings (D313's
# class, fourth instance).
#
# The citations elsewhere are deliberately NOT checked: "what the eleventh pass
# changed" is a reference to a pass, not a count of them, and a guard that
# conflated the two would force every historical mention to move.
#
# **The character class has to include the hyphen, and it did not.** The guard
# read `([a-z]+)(th|st|nd|rd)`, which matches `twentieth` and not
# `twenty-first` — so it went blank on the twenty-first pass and `expect`
# caught it. An ordinal grows a hyphen at twenty-one, which is three passes
# after the guard was written: exactly the input nobody supplies to a new check.
newest_claim=$(grep -ohE 'the newest is the [a-z-]+ pass' PASSES.md \
  | head -1 | sed -E 's/.*the newest is the ([a-z-]+) pass/\1/')
expect "PASSES.md's newest-pass claim" "$newest_claim" || :
#
# **Counting the headings was the first attempt and it was wrong**, which is why
# a guard is run before it is believed: this file caps itself at the last five
# passes in full and compresses the rest under one "Earlier passes, compressed"
# heading, so nine dated headings describe fourteen passes. There is no count to
# compare against, and inventing one would put a second home under a figure whose
# whole defect was having none.
#
# What IS checkable is the invariant that actually broke: the newest heading and
# the header have to name the same pass.
# And the newest heading has to carry that ordinal, so a pass cannot name itself
# something the file already used.
top=$(grep -E '^## [0-9]{4}-[0-9]{2}-[0-9]{2} \(' PASSES.md | head -1)
case "$top" in
  *"($newest_claim pass)"*) : ;;
  *) echo "PASSES.md's newest heading does not carry the ordinal '$newest_claim': ${top:0:80}"; fail=1 ;;
esac

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

# **Deleted 2026-09-21 with the constant it read.** This loop compared
# `policy::VERIFIED_AGAINST` against the version these notes say behaviour was
# verified against. The constant went with the permission gate (D249); the loop
# stayed, found nothing, and `continue`d — a guard that reads as installed and
# has checked nothing since the deletion.
#
# It is the orphan class (D298, D312) found for a third time and for the first
# time *outside* `src/`: after deleting a capability, grep for its vocabulary in
# the guards as well as in the code. The frozen baseline it used to hold is
# QUALITY.md's own prose now, and prose is what §4 says a figure may not be — so
# if the baseline ever becomes load-bearing again it comes back as a STATE.md row
# with a guard that `expect`s it, not as a pattern hoping a phrase survived.

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
# unavailable, so a clean offline checkout stays green; `DEVPLANE_NO_NET=1`
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
if [ -z "${DEVPLANE_NO_NET:-}" ] && command -v curl >/dev/null 2>&1; then
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
      https://github.com/hupe1980/devplane/releases/latest 2>/dev/null | sed 's|.*/tag/||')
    if [ -n "$latest" ] && [ "$latest" != "v$claimed_release" ]; then
      echo "these notes call $claimed_release released; the forge serves $latest"
      fail=1
    fi
  fi

  # 2. The rule-ledger floor guard was here until 2026-09-18, and went with the
  #    obligation it enforced. It failed when the vendor shipped past the floor,
  #    because rule-relevant rows were rows somebody owed a run on. Devplane no
  #    longer mirrors anybody's permission semantics (D249), so a changed rule
  #    shape is the vendor's business and nothing is owed.
  #
  #    What is still ours is the *hook contract*, because that is how Devplane
  #    reaches a session at all. `just channels` reads it — as a reading with a
  #    boundary rather than a floor with a debt attached, which is the shape
  #    that kept failing.

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
  #    **And on 2026-09-21 it was found reading a home the figure was never put
  #    in.** It grepped STATE.md for `npm publishes N.N.N`; that sentence is in
  #    no file. The figure that exists is the *pin* — `@github/copilot@1.0.83` —
  #    written in PROVIDERS.md and RISKS.md (R41). So the guard now reads the pin
  #    wherever the notes spell it, which is the thing R41 actually asks for: a
  #    version this project pins, against what that registry publishes today.
  #    The pin does not move automatically (D29); what this produces is a
  #    decision to make.
  pinned=$(grep -rhoE '@github/copilot@[0-9]+\.[0-9]+\.[0-9]+' *.md 2>/dev/null \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | sort -Vu | tail -1)
  expect "the pinned @github/copilot version" "$pinned" || :
  if [ -n "$pinned" ]; then
    published=$(get https://registry.npmjs.org/@github/copilot/latest \
      | grep -oE '"version":"[0-9]+\.[0-9]+\.[0-9]+"' | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
    if [ -n "$published" ] && [ "$published" != "$pinned" ]; then
      echo "note: these notes pin @github/copilot $pinned; npm publishes $published (R41 — a decision, not a failure)"
    fi
  fi
fi

# Star counts, and the rule is consistency rather than correctness.
#
# A figure about somebody else's repository cannot be recomputed here, so this
# guard cannot say whether 28,136 is right. What it can say is that the notes
# spell it **one way**, which is the rule STATE.md's own header states and the
# one thing no guard was enforcing.
#
# It was bought by the defect it now catches: Vibe Kanban's star count was
# written `28,125` in DIRECTION.md, DECISIONS.md and ROADMAP.md and `28,129` in
# README.md, RISKS.md and STATE.md — with MARKET_LANDSCAPE.md carrying *both*,
# eleven lines apart. Six files, two numbers, one repository, and 242/242 claims
# green throughout, because the ledger pins claims to fetched sources and a star
# count has no fetched source to pin to.
#
# The instrument is near-duplicate detection: two star figures that differ by
# less than 1 % are the same repository spelled twice, because no two distinct
# projects these notes cite are that close together. That is a heuristic, and it
# is the right one here — an exact-match rule cannot tell 28,136 (Vibe Kanban)
# from 32,843 (ZeroClaw), and an entity-aware rule would need to parse prose.
# Both separators the notes use: `137,999` and `137 947`. The space form was a
# fourth spelling of Spec Kit's count that the comma-only first draft of this
# guard walked straight past — which is this file's recurring lesson, that a
# guard is not installed until it has been run against a figure known to be
# wrong.
stars=$(grep -rhoE '[0-9]{1,3}([ ,][0-9]{3})+ ?(★|stars?)|[0-9]{4,} ?(★|stars?)' *.md 2>/dev/null \
  | grep -oE '[0-9][0-9, ]*[0-9]|[0-9]+' | tr -d ', ' | sort -un)
if [ -n "$stars" ]; then
  dupes=$(echo "$stars" | awk '
    { n[NR]=$1 }
    END {
      for (i=1; i<=NR; i++)
        for (j=i+1; j<=NR; j++)
          if (n[j] != n[i] && (n[j]-n[i]) < n[i]*0.01)
            printf "%s/%s ", n[i], n[j]
    }')
  if [ -n "$dupes" ]; then
    echo "one repository's star count is spelled two ways: $dupes"
    echo "  (a figure about somebody else's machine has one home in STATE.md; the rest cite it)"
    fail=1
  fi
fi

# A 📐 is a claim about `src/`, and nothing checked it.
#
# Bought by D307: PROVIDERS.md §2 marked `elicitation/create` as *designed,
# unbuilt* while the handler had shipped the day before. From that one table
# cell came a decision, a roadmap item, a promotion into *Now* and a
# twenty-two-task specification — four artefacts, none of which read the code.
# STATE.md had the shipping recorded correctly the whole time; the two files
# disagreed and nothing compared them.
#
# The instrument is exact rather than clever: the ACP client registers a handler
# per request type it answers, so the set of `on_receive_request` closures IS the
# set of protocol methods Devplane implements. A method with a handler that
# PROVIDERS.md still marks 📐 is the defect, in the one direction nobody looks.
#
# Only the mapping below is checked — adding a row costs a line and a missing
# one fails open, which is the right way round for a guard whose job is to catch
# a stale *claim* rather than to inventory the protocol.
acp_impl="../src/acp/session.rs"
if [ -f "$acp_impl" ] && [ -f PROVIDERS.md ]; then
  # request type in the handler  →  the method name the notes spell
  while IFS='|' read -r rust_type method; do
    [ -n "$rust_type" ] || continue
    grep -q "async move |request: $rust_type" "$acp_impl" || continue
    # It is implemented. The row naming it must not still say "designed".
    # **The mark, not the prose.** `| 📐` is the cell's own marker; a 📐 further
    # along a line is a row *quoting* its old state in order to retire it — which
    # this guard matched on its first run, against the very row D307 corrected.
    # The same mistake this file's other guards have each made once.
    row=$(grep -F -- "\`$method\`" PROVIDERS.md | grep -E '\| 📐' | head -1)
    if [ -n "$row" ]; then
      echo "PROVIDERS.md marks \`$method\` as 📐 and src/acp/session.rs handles it ($rust_type) — a 📐 is a claim about the tree (D307)"
      fail=1
    fi
  done <<'ACP'
CreateElicitationRequest|elicitation/create
RequestPermissionRequest|session/request_permission
ACP
fi

[ $fail = 0 ] && echo "concepts-check: ok ($(ls *.md | wc -l | tr -d ' ') files)"
exit $fail

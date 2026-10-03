#!/usr/bin/env bash
# Holds concepts/ to its own rules. A recipe, not a CI stage: concepts/ is untracked.
# Every extraction feeding a comparison goes through `expect`, so a guard that
# matches nothing fails by name instead of passing.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root/concepts" 2>/dev/null || { echo "no concepts/ directory"; exit 1; }
fail=0

expect() { # expect <guard name> <extracted value>
  # Empty and zero both fail: `grep -c` over nothing prints `0`.
  case "${2:-}" in
    '' | 0) ;;
    *) return 0 ;;
  esac
  echo "guard '$1' matched nothing: the claim it reads is absent or has been re-spelled"
  fail=1
  return 1
}

say() { printf '%-42s %s\n' "$1" "$2"; }

# ---- the corpus is the corpus ---------------------------------------------
# Named rather than globbed, so a missing document fails.
DOCS="README.md DIRECTION.md SPEC.md PRODUCT.md ARCHITECTURE.md \
UX.md DESIGN.md PROVIDERS.md MARKET.md ROADMAP.md STATE.md DECISIONS.md RISKS.md"

for f in $DOCS; do
  [ -f "$f" ] || { echo "missing document: $f"; fail=1; }
done
for f in *.md; do
  case " $DOCS " in
    *" $f "*) ;;
    *) echo "$f: not in the declared corpus — add it to DOCS or delete it"; fail=1 ;;
  esac
done
say "corpus" "$(echo $DOCS | wc -w | tr -d ' ') documents declared"

for f in $DOCS; do
  [ -f "$f" ] || continue

  # ---- links resolve ------------------------------------------------------
  targets=$(grep -o '\](\([A-Z_]*\.md\)\(#[a-z0-9-]*\)\?)' "$f" | sed 's/^](//; s/)$//; s/#.*//' | sort -u)
  for t in $targets; do
    [ -f "$t" ] || { echo "$f: broken link -> $t"; fail=1; }
  done

  # ---- nothing links into the archive ------------------------------------
  if grep -qE '\]\([^)]*\.archive-' "$f"; then
    echo "$f: links into the archive, which is unmaintained and unlinkable"
    fail=1
  fi

  # ---- header block -------------------------------------------------------
  if [ "$f" != README.md ]; then
    grep -q '^> Part of the Devplane architecture notes' "$f" \
      || { echo "$f: missing header block"; fail=1; }
    grep -q '^> Scope:' "$f" \
      || { echo "$f: missing a Scope line — one subject per document"; fail=1; }
  fi

  # ---- only the roadmap records unfinished work ---------------------------
  if [ "$f" != ROADMAP.md ]; then
    if grep -qE '^\s*- \[ \]|\bTODO\b|\bTBD\b' "$f"; then
      echo "$f: records unfinished work (belongs in ROADMAP.md)"
      fail=1
    fi
  fi
done

# ---- the roadmap's three prohibitions -------------------------------------
# Every item has a size and an exit condition; no item is finished.
items=$(grep -c '^### `#' ROADMAP.md)
expect "roadmap items" "$items" && say "roadmap items" "$items"

missing_size=$(awk '/^### `#/{a=$0; getline; if ($0 !~ /^\*\(/) print a}' ROADMAP.md)
[ -z "$missing_size" ] || { echo "roadmap items with no size:"; echo "$missing_size"; fail=1; }

settled=$(grep -c '^\*Settled when:\*' ROADMAP.md)
expect "exit conditions" "$settled" && say "exit conditions" "$settled"

# Every item in §3/§4 must carry one. §5 Later is deliberately a bullet list.
scheduled=$(awk '/^## 3\. Now/,/^## 5\. Later/' ROADMAP.md | grep -c '^### `#')
expect "scheduled items" "$scheduled"
if [ "$settled" -lt "$scheduled" ]; then
  echo "roadmap: $scheduled scheduled items but only $settled exit conditions"
  fail=1
fi
say "scheduled / settled" "$scheduled / $settled"

if grep -qiE '^### `#[a-z-]+` .*(— \*\*shipped|— \*\*done)' ROADMAP.md; then
  echo "roadmap: an item is marked shipped — finished work belongs in §6"
  fail=1
fi

# ---- the board is checked against §3, not maintained beside it ------------
board=$(awk '/^## 2\. The board/,/^---/' ROADMAP.md \
  | grep '^| \*\*[0-9]' | awk -F'|' '{print $3}' | grep -o '#[a-z-]*')
now=$(awk '/^## 3\. Now/,/^## 4\. Next/' ROADMAP.md | grep -o '^### `#[a-z-]*`' | sed 's/^### `//; s/`//')
expect "board anchors" "$board" || true
expect "now anchors" "$now" || true
if [ "$(echo "$board")" != "$(echo "$now")" ]; then
  echo "roadmap: the board in §2 and the items in §3 disagree"
  diff <(echo "$board") <(echo "$now") | sed 's/^/  /'
  fail=1
fi
say "board == §3" "$(echo "$board" | wc -l | tr -d ' ') anchors, in order"

# ---- every anchor referenced anywhere resolves ----------------------------
declared=$(grep -oE '^### `#[a-z-]+`' ROADMAP.md | sed 's/^### `//; s/`//'; \
           awk '/^## 5\. Later/,/^## 7\./' ROADMAP.md | grep -o '`#[a-z-]*`' | sed 's/`//g'; \
           awk '/^## 6\. Retired/,/^## 7\./' ROADMAP.md | grep -o '`#[a-z-]*`' | sed 's/`//g')
expect "declared anchors" "$declared" || true
declared=$(echo "$declared" | sort -u)
used=$(grep -ho '`#[a-z-]\{3,\}`' $DOCS | sed 's/`//g' \
  | grep -vE '^#[0-9a-f]{6}$' | sort -u)
for a in $used; do
  echo "$declared" | grep -qx "$a" || { echo "unknown roadmap anchor cited: \`$a\`"; fail=1; }
done
say "anchors" "$(echo "$declared" | wc -l | tr -d ' ') declared, all citations resolve"

# ---- every §N cross-reference resolves to a section that exists ------------
# A link that resolves can still point at a section that no longer exists.
secrefs=0; badrefs=""
for f in $DOCS; do
  [ -f "$f" ] || continue
  # "[OTHER.md](OTHER.md) §3a"  ->  does OTHER.md have a "## 3a." heading?
  while read -r ref; do
    [ -n "$ref" ] || continue
    tgt=${ref%%|*}; sec=${ref##*|}
    secrefs=$((secrefs+1))
    grep -qE "^#{2,3} ${sec}[.a-z]*\b|^#{2,3} ${sec}\." "$tgt" 2>/dev/null \
      || badrefs="$badrefs\n  $f -> $tgt §$sec"
  done <<EOF
$(grep -o '\[[A-Z_]*\.md\]([A-Z_]*\.md) §[0-9][0-9a-z]*' "$f" \
  | sed 's/^\[[A-Z_]*\.md\](//; s/) §/|/')
EOF
done
expect "section references" "$secrefs" || true
if [ -n "$badrefs" ]; then
  printf "cross-references to sections that do not exist:%b\n" "$badrefs"
  fail=1
fi
say "section refs" "$secrefs checked, all resolve"

# ---- decision ids are unique and every citation resolves ------------------
ids=$(grep -oE '^### D[0-9]{3}' DECISIONS.md | sed 's/^### //')
expect "decision ids" "$ids" || true
dupes=$(echo "$ids" | sort | uniq -d)
[ -z "$dupes" ] || { echo "duplicate decision ids: $dupes"; fail=1; }
for d in $(grep -ho '\bD[0-9]\{3\}\b' $DOCS | sort -u); do
  echo "$ids" | grep -qx "$d" || { echo "citation to unknown decision: $d"; fail=1; }
done
say "decisions" "$(echo "$ids" | wc -l | tr -d ' ') unique, all citations resolve"

# A live specification may not cite a decision the ledger does not have: the
# previous ledger was renumbered, and its ids now name different decisions.
if [ -d "$root/specs" ]; then
  spec_cites=$(find "$root/specs" -path "$root/specs/archive" -prune -o -name '*.md' -print \
    | xargs grep -ho '\bD[0-9]\{3\}\b' 2>/dev/null | sort -u)
  expect "decision ids cited by live specs" "$spec_cites" || true
  for d in $spec_cites; do
    echo "$ids" | grep -qx "$d" || { echo "a specification cites an unknown decision: $d"; fail=1; }
  done
  say "spec citations" "$(echo "$spec_cites" | grep -c . | tr -d ' ') decision ids cited by live specs, all resolve"
fi

# ---- the first figure compared to the code --------------------------------
# STATE.md's version row must name the version Cargo.toml declares.
cargo_version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
expect "version in Cargo.toml" "$cargo_version" || true
state_version=$(grep '^| Version |' STATE.md | grep -o '`[0-9][0-9.]*`' | head -1 | tr -d '`')
expect "version in STATE.md" "$state_version" || true
if [ "$cargo_version" != "$state_version" ]; then
  echo "STATE.md says version $state_version; Cargo.toml says $cargo_version"
  fail=1
else
  say "version" "STATE.md matches Cargo.toml: $cargo_version"
fi
# ...and the last tag it names must be the repository's last tag.
git_tag=$(git -C "$root" tag --list 'v*' | sort -V | tail -1)
expect "last tag in git" "$git_tag" || true
state_tag=$(grep '^| Version |' STATE.md | grep -o '`v[0-9][0-9.]*` the last tag' | head -1 | cut -d'`' -f2)
expect "last tag in STATE.md" "$state_tag" || true
if [ "$git_tag" != "$state_tag" ]; then
  echo "STATE.md says the last tag is $state_tag; git says $git_tag"
  fail=1
else
  say "last tag" "STATE.md matches git: $git_tag"
fi

# ---- every figure has one home --------------------------------------------
# A number with a unit outside STATE.md must link to STATE.md or be a size the
# document owns. Figures are read off STATE.md §1–§2, not typed here.
figs=$(awk '/^## 1\./,/^## 3\./' STATE.md \
  | grep -oE '\*\*[0-9]{1,3}(,[0-9]{3})+' | tr -d '*' | sort -u)
expect "figures in STATE.md §1–2" "$figs" || true
homeless=""
for fig in $figs; do
  for w in $(grep -l -- "$fig" $DOCS 2>/dev/null | grep -v '^STATE.md$'); do
    grep -- "$fig" "$w" | grep -q 'STATE.md' || homeless="$homeless $w:$fig"
  done
done
[ -z "$homeless" ] || { echo "figures restated without citing their home:$homeless"; fail=1; }
say "figures" "$(echo "$figs" | wc -w | tr -d ' ') tree figures, every restatement cites STATE.md"

# ---- STATE.md rows that are facts about somebody else carry a date --------
# A row must name both a date and the instrument that measured it.
undated=$(awk '/^## 3\. Facts about/,/^## 4\./' STATE.md \
  | grep '^| ' | grep -v '^| Fact' | grep -v '^|---' \
  | grep -vE '20[0-9]{2}-[0-9]{2}-[0-9]{2}' | sed 's/|.*//')
uninstrumented=$(awk -F'|' '/^## 3\. Facts about/,/^## 4\./ {
    if ($0 ~ /^\| / && $0 !~ /^\| Fact/ && $0 !~ /^\|---/) {
      d=$4; gsub(/^ +| +$/,"",d);
      if (d ~ /^20[0-9-]+$/) { k=$2; gsub(/^ +| +$/,"",k); print "  " k }
    }}' STATE.md)
if [ -n "$uninstrumented" ]; then
  echo "STATE.md §3 rows carrying a date but not naming what measured them:"
  echo "$uninstrumented"
  fail=1
fi
expect "state §3 rows" "$(awk '/^## 3\. Facts about/,/^## 4\./' STATE.md | grep -c '^| ')" || true
[ -z "$undated" ] || { echo "STATE.md §3 rows with no fetch date:"; echo "$undated"; fail=1; }
say "external facts" "every row in STATE.md §3 is dated and names its instrument"

# ---- the first guard that reads the code ----------------------------------
# The notes' list of decision authorities must match `src/core/decision.rs`.
cd "$root" || exit 1
code_auth=$(sed -n '/fn as_str/,/^    }/p' src/core/decision.rs \
  | grep -o 'Authority::[A-Za-z]* => "[a-z]*"' | sed 's/.*"\(.*\)"/\1/' | sort | tr '\n' ' ')
expect "authority values in the code" "$code_auth" || true
notes_auth=$(grep -ho '`person`, `rule`, `[a-z]*`, `[a-z]*`, `[a-z]*`' concepts/*.md .specify/memory/constitution.md 2>/dev/null \
  | head -1 | grep -o '[a-z]\{4,\}' | sort | tr '\n' ' ')
expect "authority values in the notes" "$notes_auth" || true
if [ "$code_auth" != "$notes_auth" ]; then
  echo "the notes and src/core/decision.rs disagree about the authorities:"
  echo "  code:  $code_auth"
  echo "  notes: $notes_auth"
  fail=1
fi
say "authorities" "notes match src/core/decision.rs: $code_auth"

# Every `tests/<name>.rs` the notes or constitution name must exist, or be
# acknowledged somewhere in the corpus as not existing.
missing_tests=""
# Fenced blocks are stripped: a path in a code sample is an illustration.
strip_fences() { awk 'BEGIN{inf=0} /^[[:space:]]*```/{inf=!inf; next} !inf' "$@"; }
named_tests=$(strip_fences concepts/*.md .specify/memory/constitution.md 2>/dev/null \
  | grep -o 'tests/[a-z_]*\.rs' | sort -u)
expect "test files named in the notes" "$named_tests" || true
for tf in $named_tests; do
  [ -f "$tf" ] && continue
  # Acknowledgement may be anywhere in the corpus, not only on the same line.
  if ! strip_fences concepts/*.md .specify/memory/constitution.md 2>/dev/null \
     | grep -- "$tf" | grep -q -E 'does not exist|not yet written|not exist yet|is the first item'; then
    missing_tests="$missing_tests\n  $tf  (named, absent, and nowhere acknowledged)"
  fi
done
if [ -n "$missing_tests" ]; then
  printf "the notes name test files that do not exist and do not say so:%b\n" "$missing_tests"
  fail=1
fi
say "named tests" "$(echo "$named_tests" | wc -l | tr -d ' ') named, each exists or is marked absent"

# The Verdict variants the notes rely on must still exist in the code.
code_verdict=$(sed -n '/pub enum Verdict/,/^}/p' src/core/policy.rs \
  | grep -oE '^    [A-Z][A-Za-z]*' | tr -d ' ' | sort | tr '\n' ' ')
expect "Verdict variants in the code" "$code_verdict" || true
for v in Deny Ask Unresolved Undecided; do
  echo "$code_verdict" | grep -qw "$v" || { echo "Verdict lost the $v variant"; fail=1; }
done
grep -q 'Unresolved' concepts/STATE.md || { echo "STATE.md stopped naming Unresolved"; fail=1; }
say "verdict" "code has: $code_verdict"

# The absent `classifier` authority must stay absent.
if grep -q 'Classifier' src/core/decision.rs; then
  echo "src/core/decision.rs grew a Classifier authority; the notes say there is none"
  fail=1
fi

# Any published line naming `classifier` beside two or more real authorities is a
# wrong enumeration; lines saying it is absent are fine. Wire types, spec fixtures
# and this script are exempt.
published="src ui/src tests site/content site/static site/templates site/zola.toml scripts README.md CONTRIBUTING.md"
inc="--include=*.rs --include=*.ts --include=*.svelte --include=*.md --include=*.html --include=*.txt --include=*.toml --include=*.sh"
not_ours='^ui/src/wire/|^tests/fixtures/|^scripts/concepts-check\.sh:'
auth_words='\b(person|rule|timer|nobody|daemon)\b'
badlist=$(grep -rni $inc -- 'classifier' $published 2>/dev/null \
  | grep -vE "$not_ours" \
  | grep -viE 'no `?classifier|not on that list|deliberately' \
  | while IFS= read -r hit; do
      text=${hit#*:}; text=${text#*:}
      n=$(printf '%s\n' "$text" | grep -oiE "$auth_words" | tr '[:upper:]' '[:lower:]' | sort -u | wc -l | tr -d ' ')
      [ "$n" -ge 2 ] && echo "$hit"
    done || true)
if [ -n "$badlist" ]; then
  echo "an authority enumeration names 'classifier', which src/core/decision.rs does not have:"
  echo "$badlist" | sed 's/^/  /'
  fail=1
fi
enums=$(grep -rlE 'a person, a rule, a' $published 2>/dev/null | grep -vE "$not_ours" | wc -l | tr -d ' ')
expect "authority enumerations in the published tree" "$enums" || true
say "published authorities" "$enums enumerations, none names a value the code lacks"
cd "$root/concepts" || exit 1

# ---- an instrument must name something that exists ------------------------
# Every path an instrument cites must exist: an absence claim checked in the wrong
# place reads the same as a true one.
cd "$root" || exit 1
ghosts=""; paths_checked=0
for pth in $(awk '/^## 2\. The tree/,/^## 3\./' concepts/STATE.md \
    | grep -oE '(src|ui|tests|scripts|site)/[A-Za-z0-9_./-]+' | sort -u); do
  paths_checked=$((paths_checked+1))
  [ -e "$pth" ] && continue
  # absent is honest only where the row is *about* that absence
  grep -q -- "$pth" concepts/STATE.md && \
    grep -- "$pth" concepts/STATE.md | grep -q -E 'does not exist|never existed|no test' \
    || ghosts="$ghosts\n  $pth"
done
expect "paths named by instruments" "$paths_checked" || true
if [ -n "$ghosts" ]; then
  printf "STATE.md names paths that do not exist and does not say so:%b\n" "$ghosts"
  fail=1
fi
say "instruments" "$paths_checked paths named, each exists or is declared absent"
cd "$root/concepts" || exit 1

# ---- the published tree may not cite these notes ---------------------------
# A `concepts/…` path or bare `D…`/`R…` id is always a citation. Spec Kit ids
# (`FR-001`, `T041`, `NNN-slug`) are also legitimate examples, so they are caught
# only in prose: code comments and unfenced doc lines, backticked spans removed.
cd "$root" || exit 1
leak=$(grep -rn $inc -E 'concepts/[A-Z]|\bD[0-9]{3}\b|\bR[0-9]{1,2}\b' $published 2>/dev/null \
  | grep -vE "$not_ours" || true)
if [ -n "$leak" ]; then
  echo "the published tree cites the notes:"
  echo "$leak" | sed 's/^/  /'
  fail=1
fi

bt=$(printf '\140'); fence="$bt$bt$bt"
spec_ids='\b(FR|SC)-[0-9]{3}[a-z]?\b|\bT[0-9]{3}\b|\b0[0-9]{2}-[a-z][a-z-]+\b'
# A function because a `)` in a case pattern inside `$( )` confuses the parser.
cites_a_spec_id() { # <file:line:text>  -> prints it when the citation is prose
  local hit=$1 file rest line text opens
  file=${hit%%:*}; rest=${hit#*:}; line=${rest%%:*}; text=${rest#*:}
  case "$file" in
    *.rs|*.ts|*.svelte|*.sh|*.toml)
      # Code: a comment line is prose; anything else is data.
      printf '%s' "$text" | grep -qE '^[[:space:]]*(//|#|\*|<!--)' || return 0 ;;
    *)
      # A document: a line inside a fenced block is an illustration.
      opens=$(head -n $((line - 1)) "$file" 2>/dev/null | grep -c "^[[:space:]]*$fence")
      [ $((opens % 2)) -eq 1 ] && return 0 ;;
  esac
  # Backticked spans are examples; "2 000-character" is a number, not a folder.
  printf '%s' "$text" | sed -E "s/$bt[^$bt]*$bt//g; s/[0-9][[:space:]]0[0-9]{2}-[a-z-]+//g" \
    | grep -qE "$spec_ids" && echo "$hit"
  return 0
}
ids=$(grep -rn $inc -E "$spec_ids" $published 2>/dev/null | grep -vE "$not_ours" \
  | while IFS= read -r hit; do cites_a_spec_id "$hit"; done || true)
if [ -n "$ids" ]; then
  echo "the published tree cites a specification's identifiers:"
  echo "$ids" | sed 's/^/  /'
  fail=1
fi
say "published tree" "cites no note and no specification id"

# This checkout's own change folders by exact name, backticked or not.
# Only a working checkout has `specs/`; a clean clone says so.
if [ -d specs ]; then
  features=0
  for feature in specs/[0-9]*/; do
    [ -d "$feature" ] || continue
    name=$(basename "$feature"); features=$((features + 1))
    hits=$(grep -rn $inc --fixed-strings -- "$name" $published 2>/dev/null | grep -vE "$not_ours" || true)
    if [ -n "$hits" ]; then
      echo "the published tree names a gitignored specification ($name):"
      echo "$hits" | sed 's/^/  /'
      fail=1
    fi
  done
  expect "change folders under specs/" "$features" || true
  say "specifications" "$features change folders, none named in the published tree"
else
  say "specifications" "skipped: specs/ absent, so nothing could have been named"
fi

# ---- prose about the code, compared to the code (D080) --------------------
cd "$root/concepts" || exit 1
# The sprint of 2026-10-02 found fourteen wrong statements under an all-green
# run: each was a claim in prose no guard read. These read the cheapest kinds.
# Run from concepts/ like the guards above; fenced blocks are illustrations.
prose() { awk '/^```/{f=!f; next} !f' "$1"; }

# A `src/…/` or `ui/…/` directory named in the notes must exist, unless the
# same line says it does not.
missing_dirs=""
for f in $DOCS; do
  [ -f "$f" ] || continue
  for d in $(prose "$f" | grep -vE 'not exist|does not|absent|deleted|removed|no longer' \
               | grep -oE '`(src|ui/src|tests)/[A-Za-z0-9_/.-]+/`' | tr -d '`' | sort -u); do
    [ -d "../$d" ] || missing_dirs="$missing_dirs\n  $f names $d"
  done
done
[ -z "$missing_dirs" ] || { printf '%b\n' "directories the notes name that do not exist:$missing_dirs"; fail=1; }
say "directories" "every src/ui/tests directory the notes name exists"

# "N dialects" outside the roadmap and the log must be the number the code models.
num() { case "$1" in one) echo 1;; two) echo 2;; three) echo 3;; four) echo 4;; five) echo 5;; six) echo 6;; seven) echo 7;; eight) echo 8;; *) echo "$1";; esac; }
dialects=$(awk '/^pub const DIALECTS/,/^\];/' ../src/spec.rs | grep -c 'name: "')
expect "dialects in src/spec.rs" "$dialects" || true
bad_dialects=""
for f in $DOCS; do
  case "$f" in ROADMAP.md|DECISIONS.md) continue ;; esac
  [ -f "$f" ] || continue
  while read -r w; do
    [ -n "$w" ] || continue
    [ "$(num "$w")" = "$dialects" ] || bad_dialects="$bad_dialects\n  $f says $w"
  # Any number word directly before "dialects" is a count, wherever the line
  # cites the retired anchor; "two dialects in one repository" is not a count.
  done <<< "$(prose "$f" | grep -viE 'would|will|unbuilt' \
               | grep -oiE '\b(one|two|three|four|five|six|seven|eight|[0-9]+) (spec )?dialects\b( in one)?' \
               | grep -viE ' in one$' | awk '{print tolower($1)}')"
done
# STATE's own row says it in a cell: "Spec dialects modelled | **four, …".
state_dialects=$(grep '^| Spec dialects modelled |' STATE.md | grep -oiE '^\| Spec dialects modelled \| \*\*[a-z0-9]+' \
  | sed 's/.*\*\*//' | tr '[:upper:]' '[:lower:]')
expect "dialect count in STATE.md's row" "$state_dialects" || true
if [ -n "$state_dialects" ] && [ "$(num "$state_dialects")" != "$dialects" ]; then
  bad_dialects="$bad_dialects\n  STATE.md's row says $state_dialects"
fi
[ -z "$bad_dialects" ] || { printf '%b\n' "dialect counts that are not the code's ($dialects):$bad_dialects"; fail=1; }
say "dialects" "code models $dialects; every count outside the roadmap, STATE's row included, agrees"

# "N top-level commands" must be the number COMMAND_GROUPS lists.
commands=$(awk '/^pub const COMMAND_GROUPS/,/^\];/' ../src/cli/mod.rs | grep -oE '&\[[^]]*\]|^ +"[a-z]+",' | grep -oE '"[a-z]+"' | wc -l | tr -d ' ')
expect "commands in COMMAND_GROUPS" "$commands" || true
bad_cmds=""
for f in $DOCS; do
  case "$f" in DECISIONS.md) continue ;; esac
  [ -f "$f" ] || continue
  for n in $(prose "$f" | grep -oE '\*?\*?[0-9]+\*?\*? top-level commands' | grep -oE '[0-9]+'); do
    [ "$n" = "$commands" ] || bad_cmds="$bad_cmds\n  $f says $n"
  done
done
[ -z "$bad_cmds" ] || { printf '%b\n' "top-level command counts that are not COMMAND_GROUPS' ($commands):$bad_cmds"; fail=1; }
say "commands" "COMMAND_GROUPS lists $commands; every count in the notes agrees"

# A retired anchor cited outside the roadmap and the log must be cited as retired.
# Only the retired anchor itself — not the live anchor it points to (`→ #x`).
retired=$(awk '/^## 6\. Retired/,/^## 7\./' ROADMAP.md | sed 's/([^)]*)//g' | grep -oE '`#[a-z0-9-]+`' | tr -d '`' | sort -u)
expect "retired anchors" "$(echo "$retired" | grep -c .)" || true
live_retired=""
for f in $DOCS; do
  case "$f" in ROADMAP.md|DECISIONS.md) continue ;; esac
  [ -f "$f" ] || continue
  for a in $retired; do
    hit=$(prose "$f" | grep -nF "\`$a\`" | grep -viE 'retired|shipped|deleted|folded|→|merged|absorbed|replaced|was ' || true)
    [ -z "$hit" ] || live_retired="$live_retired\n  $f cites $a as live: $(echo "$hit" | head -1 | cut -c1-80)"
  done
done
[ -z "$live_retired" ] || { printf '%b\n' "retired anchors cited as live work:$live_retired"; fail=1; }
say "retired anchors" "none cited as live work outside the roadmap and the log"

cd "$root" || exit 1

# The store's schema version the notes state must be the code's.
cd "$root" || exit 1
code_schema=$(grep -oE 'pub const SCHEMA_VERSION: i64 = [0-9]+' src/store.rs | grep -oE '[0-9]+$')
expect "SCHEMA_VERSION in src/store.rs" "$code_schema" || true
notes_schema=$(grep -oE '`SCHEMA_VERSION` [0-9]+' concepts/STATE.md | grep -oE '[0-9]+$' | sort -u)
expect "SCHEMA_VERSION in STATE.md" "$notes_schema" || true
if [ -n "$notes_schema" ] && [ "$notes_schema" != "$code_schema" ]; then
  echo "STATE.md says the store schema is $notes_schema; src/store.rs says $code_schema"
  fail=1
fi
say "schema" "STATE.md matches src/store.rs: $code_schema"

echo
if [ "$fail" = 0 ]; then
  echo "ok — and every guard above found something to compare"
else
  echo "FAILED"
fi
exit "$fail"

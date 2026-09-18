#!/usr/bin/env bash
# Measure the releases between the measured floor and the vendor's head, using
# only the probes those releases' own changelog rows name.
#
# The third floor — `policy::ROWS_MEASURED_THROUGH` — says *every rule row the
# vendor announced up to here produced a probe the running vendor agreed with*.
# It sits between the cheap read floor (a person dispositioned every row) and
# the expensive compatibility floor (the whole matrix agreed), because the claim
# underneath it does.
#
#   scripts/measured-through.sh --dry-run     # resolve the span, spend nothing
#   scripts/measured-through.sh [--to X]      # ask the running product
#
# Exit 0 green · 1 red (a disagreement) · 2 incomplete (a precondition was
# missing — it says which, and changes nothing).
#
# **This script cannot advance the compatibility floor.** It has no code path to
# `VERIFIED_AGAINST`, and that is a property with a test rather than a promise
# in a comment: a cheap run wearing an expensive claim is the failure the whole
# three-floor design exists to prevent.
set -u
cd "$(dirname "$0")/.." || exit 2

CHANGELOG="concepts/reference/claude-code/CHANGELOG.md"
LEDGER="scripts/changelog-ledger.txt"
PROBES="scripts/probes.txt"
POLICY="src/core/policy.rs"
RECORDS="scripts/measurements"

DRY=0; TO=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY=1 ;;
    --to) shift; TO="${1:-}" ;;
    *) echo "measured-through: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

for f in "$CHANGELOG" "$LEDGER" "$PROBES" "$POLICY"; do
  [ -f "$f" ] || { echo "measured-through: $f is missing — nothing measured, nothing changed"; exit 2; }
done

floor=$(grep -oE 'pub const ROWS_MEASURED_THROUGH: &str = "[0-9.]+"' "$POLICY" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
[ -n "$floor" ] || { echo "measured-through: cannot read the measured floor from $POLICY"; exit 2; }
head_ver=$(grep -m1 -oE '^## [0-9]+\.[0-9]+\.[0-9]+' "$CHANGELOG" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
[ -n "$head_ver" ] || { echo "measured-through: cannot read the vendor head from $CHANGELOG"; exit 2; }
[ -n "$TO" ] || TO="$head_ver"

# The releases in the span, oldest first. A release with no heading between the
# floor and `TO` simply does not exist and contributes nothing.
span=$(awk -v floor="$floor" -v to="$TO" '
  /^## [0-9]+\.[0-9]+\.[0-9]+/ {
    v = $2
    split(v, a, "."); split(floor, f, "."); split(to, t, ".")
    if (a[1] "." a[2] != f[1] "." f[2]) next
    if (a[3]+0 > f[3]+0 && a[3]+0 <= t[3]+0) print v
  }' "$CHANGELOG" | sort -t. -k3 -n)

if [ -z "$span" ]; then
  echo "measured-through: the measured floor is $floor and nothing newer is in the span up to $TO"
  exit 0
fi

# ── Resolve each release's rows from the ledger ──────────────────────────────
#
# Row text is somebody else's prose. It is carried as a quoted report and is
# never interpolated into a command and never turned into a probe: a person
# writes the probe reference, once, into the ledger.
rows_for() { # release -> "<disposition>\t<probe id or ->\t<text>"
  # Split on a literal `|` with -F, never on `" | "` as a pattern: awk reads a
  # split separator as a regex, so `|` there is alternation and every field
  # comes back wrong — which it did, silently, and reported nothing owed.
  awk -F'|' -v ver="$1" '
    index($0, ver " | ") == 1 {
      disp = $2; gsub(/^[ \t]+|[ \t]+$/, "", disp)
      text = $0; sub(/^[^|]*\|[^|]*\| ?/, "", text)
      id = "-"
      if ((getline nxt) > 0 && nxt ~ /\^ probe:/) {
        match(nxt, /probe:[a-z0-9][a-z0-9_-]*/)
        id = substr(nxt, RSTART+6, RLENGTH-6)
      }
      printf "%s\t%s\t%s\n", disp, id, text
    }' "$LEDGER"
}

detail_file=$(mktemp); trap 'rm -f "$detail_file"' EXIT
ids=""; owed_releases=""; norow_releases=""; probed_releases=""
# Per-release detail for the record. The contract asks for the releases, their
# outcomes and their rows — not just a total — because a record that says
# "3 agreed" cannot answer *which release is now measured*, which is the only
# question it exists to answer later.
detail=""
for rel in $span; do
  rows=$(rows_for "$rel")
  if [ -z "$rows" ]; then
    norow_releases="$norow_releases $rel"
    # `no-rows` is recorded as its own outcome, never as a clean measurement.
    printf '%s\t%s\t%s\t%s\t%s\n' "$rel" "no-rows" "-" "-" "" >> "$detail_file"
    continue
  fi
  rel_owed=0
  while IFS=$'\t' read -r disp id _text; do
    case "$disp" in
      probe) ids="$ids${ids:+,}$id" ;;
      case) rel_owed=1 ;;
      declined) : ;;
    esac
  done <<< "$rows"
  if [ "$rel_owed" = 1 ]; then
    owed_releases="$owed_releases $rel"
    rel_outcome=owed
  else
    probed_releases="$probed_releases $rel"
    rel_outcome=measured
  fi
  while IFS=$'\t' read -r disp id text; do
    # Row text is somebody else's prose: carried as data, never interpolated.
    printf '%s\t%s\t%s\t%s\t%s\n' "$rel" "$rel_outcome" "$disp" "$id" "$text" >> "$detail_file"
  done <<< "$rows"
done

echo "measured-through: floor $floor -> attempting $TO"
echo "  span            $(echo "$span" | tr '\n' ' ')"
echo "  announced none  ${norow_releases:-(none)}"
echo "  fully probed    ${probed_releases:-(none)}"
echo "  owed (a row has only a test over our own matcher, not a probe) ${owed_releases:-(none)}"
echo "  probes to run   ${ids:-(none)}"

# The floor stops **below** the first owed release, and every release above it
# is unmeasured regardless of its own outcome. Written as a break rather than a
# filter on purpose: "skip and continue" would advance a floor over a gap.
stop_at="$floor"
for rel in $span; do
  case " $owed_releases " in *" $rel "*) break ;; esac
  stop_at="$rel"
done
echo "  floor could reach $stop_at"

# Whether every probe this span would run still resolves to a shape the harness
# can execute. Asked here, for free, rather than discovered halfway through a
# run somebody is paying for.
if [ -n "$ids" ]; then
  if DEVPLANE_DIFF_CHECK=1 DEVPLANE_DIFF_PROBES="$ids" bash scripts/verify-permissions-diff.sh >/dev/null 2>&1; then
    echo "  registry        every probe resolves to a runnable shape"
  else
    echo "  registry        A PROBE NO LONGER RESOLVES — run with DEVPLANE_DIFF_CHECK=1 to see which"
    [ "$DRY" = 1 ] || exit 2
  fi
fi

if [ "$DRY" = 1 ]; then
  echo "measured-through: dry run — nothing was asked and nothing changed"
  exit 0
fi

# ── Preconditions for spending ──────────────────────────────────────────────
command -v claude >/dev/null 2>&1 || {
  echo "measured-through: no \`claude\` on PATH — nothing measured, no floor changed"; exit 2; }
observed=$(claude --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
[ -n "$observed" ] || {
  echo "measured-through: \`claude --version\` said nothing — nothing measured, no floor changed"; exit 2; }
# The version that *answered*, not the one somebody meant to measure. The
# harness has been wrong about exactly this before: a binary chosen by glob order.
echo "  vendor answering $observed"

# A binary cannot demonstrate behaviour it predates.
#
# The floor says *every row announced up to here was probed against the running
# vendor*. If the installed binary is older than the release the floor would
# reach, that sentence is false in the most dangerous direction: it would claim
# agreement about a change the binary does not contain. Found by running this
# for the first time against a machine whose `claude` was nine releases behind
# the head — the dry run said nothing, because a dry run never asks who answers.
older() { # a b -> 0 when a < b, same minor series only
  awk -v a="$1" -v b="$2" 'BEGIN{
    split(a,x,"."); split(b,y,".");
    if (x[1]!=y[1] || x[2]!=y[2]) exit 1;
    exit (x[3]+0 < y[3]+0) ? 0 : 1 }'
}
if older "$observed" "$stop_at"; then
  echo "measured-through: the installed vendor is $observed and the floor would reach $stop_at"
  echo "  a binary cannot demonstrate behaviour it predates — nothing measured, no floor changed"
  exit 2
fi
if older "$observed" "$TO"; then
  echo "  NOTE            $observed is behind the span top $TO; rows announced after $observed"
  echo "                  are not measurable here, and the floor stops accordingly"
fi

if [ -z "$ids" ]; then
  echo "measured-through: no probes to run in this span"
  out=""; rc=0
else
  out=$(DEVPLANE_DIFF_PROBES="$ids" bash scripts/verify-permissions-diff.sh 2>&1); rc=$?
  printf '%s\n' "$out" | grep '^PROBE ' || true
fi

agreed=$(printf '%s\n' "$out" | grep -c '^PROBE .* agreed' || true)
declared=$(printf '%s\n' "$out" | grep -c '^PROBE .* declared' || true)
skipped=$(printf '%s\n' "$out" | grep -c '^PROBE .* skipped' || true)
disagreed=$(printf '%s\n' "$out" | grep -c '^PROBE .* disagreed' || true)

verdict=green
[ "$skipped" -gt 0 ] && verdict=incomplete
[ "$disagreed" -gt 0 ] && verdict=red
[ "$rc" -ge 2 ] && verdict=incomplete

# A skipped probe makes its release owed, so a green verdict with a skip in it
# is unrepresentable rather than merely discouraged.
floor_after="$stop_at"
[ "$verdict" = green ] || floor_after="$floor"

mkdir -p "$RECORDS"
record="$RECORDS/$TO.json"
probe_lines=$(printf '%s\n' "$out" | grep '^PROBE ' || true)
python3 scripts/write-measurement.py "$record" "$floor" "$TO" "$observed" "$verdict" "$floor_after" \
         "$detail_file" "$probe_lines"

echo "measured-through: $verdict — $agreed agreed, $declared declared, $skipped skipped, $disagreed disagreed"
echo "measured-through: this measured only what those releases announced; it says nothing about the rest of the matcher"
case "$verdict" in
  green)
    echo "measured-through: the measured floor may move $floor -> $floor_after"
    echo "  edit ROWS_MEASURED_THROUGH in $POLICY in the same commit as $record"
    exit 0 ;;
  red) echo "measured-through: no floor moves on a disagreement"; exit 1 ;;
  *)   echo "measured-through: incomplete — a skipped probe is unmeasured, not clean; no floor moves"; exit 2 ;;
esac

#!/usr/bin/env bash
# The dependency count, recomputed rather than remembered.
# Measured with `cargo tree` (what is actually compiled) for one pinned target, since
# the graph differs per host. Two measures, for the default CLI and `--features app`:
#   total — package versions the binary builds (normal edges)
#   sqlx  — of those, how many are reachable only through sqlx
# Exits 1 if either drifts from the figures pinned below.
set -u
cd "$(dirname "$0")/.." || exit 1

# The pinned figures.
TARGET=x86_64-unknown-linux-musl
WANT_TOTAL=215
WANT_ONLY_SQLX=28
WANT_TOTAL_APP=414
WANT_ONLY_SQLX_APP=16

tree=$(mktemp)
trap 'rm -f "$tree"' EXIT

# count <label> <want total> <want only-sqlx> <extra cargo flags…>
count() {
  local label=$1 want_total=$2 want_only=$3
  shift 3
  cargo tree --edges normal --prefix depth --no-dedupe --target "$TARGET" "$@" >"$tree" 2>/dev/null
  [ -s "$tree" ] || { echo "cargo tree failed ($label)"; return 1; }

  echo "  [$label] target                     $TARGET"
  # The tree goes in by path, not on stdin: the heredoc below is already stdin.
  python3 - "$want_total" "$want_only" "$tree" "$label" <<'PY'
import re, sys

# `-` means print the figure without comparing it (see the app count below).
compare = sys.argv[1] != "-"
want_total, want_only = (int(x) for x in sys.argv[1:3]) if compare else (0, 0)
label = sys.argv[4]
rows = []
for line in open(sys.argv[3]).read().splitlines():
    m = re.match(r"^(\d+)(.*)$", line)
    if not m:
        continue
    depth = int(m.group(1))
    # `name v1.2.3 (proc-macro)` / `… (/path)` / a trailing `(*)` for a repeat.
    pkg = m.group(2).strip().removesuffix(" (*)")
    pkg = re.sub(r" \((proc-macro|/[^)]*)\)$", "", pkg).strip()
    rows.append((depth, pkg))

root = rows[0][1]

def reachable(cut=None):
    """Package versions reachable from the root, optionally cutting one edge."""
    seen, stack = set(), []          # stack[d] is the package at depth d
    out, skipping = set(), None
    for depth, pkg in rows:
        del stack[depth:]
        stack.append(pkg)
        if skipping is not None:
            if depth > skipping:
                continue             # still inside the cut subtree
            skipping = None
        parent = stack[depth - 1] if depth else None
        if cut and parent == cut[0] and pkg.split()[0] == cut[1]:
            skipping = depth
            continue
        out.add(pkg)
    return out

total = reachable()
without = reachable(cut=(root, "sqlx"))
only_sqlx = len(total) - len(without)

print(f"  [{label}] package versions actually built    {len(total)}")
print(f"  [{label}] reachable only through sqlx        {only_sqlx}")
print(f"  [{label}] without sqlx                       {len(without)}")

if not compare:
    sys.exit(0)
bad = 0
for name, got, want in (("total", len(total), want_total),
                        ("only-sqlx", only_sqlx, want_only)):
    if got != want:
        print(f"DRIFT {label} {name}: pinned at {want}, the build says {got}")
        bad = 1
sys.exit(bad)
PY
}

rc=0
count default "$WANT_TOTAL" "$WANT_ONLY_SQLX" || rc=1
# Proc-macros are built for the host, so `cargo tree` resolves their
# dependencies against the host whatever `--target` says: Tauri's macros pull
# in two more packages on macOS than on Linux. The app figures are Linux's —
# what CI measures — and are compared only there; elsewhere they are printed
# and the comparison is skipped by name rather than passed.
if [ "$(uname -s)" = Linux ]; then
  count app "$WANT_TOTAL_APP" "$WANT_ONLY_SQLX_APP" --features app || rc=1
  [ $rc -eq 0 ] && echo "deps-count: ok (matches the pinned figures)"
else
  count app - - --features app || rc=1
  echo "  [app] not compared: the pinned figures ($WANT_TOTAL_APP, $WANT_ONLY_SQLX_APP) are a Linux host's; this is $(uname -s)"
  [ $rc -eq 0 ] && echo "deps-count: ok (default matches; app is compared on Linux)"
fi
exit $rc

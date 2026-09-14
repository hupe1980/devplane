#!/usr/bin/env bash
# The dependency numbers in D70 and D78, recomputed rather than remembered.
#
# Measured from `cargo tree`, not from `cargo metadata`'s resolve graph: the
# latter lists every dependency a package *could* have, including optional ones
# no enabled feature turns on, which this binary never compiles.
#
# Counted for one pinned target, because the graph is host-dependent: macOS
# resolves one package fewer than Linux and two fewer than Windows, so an
# unpinned count asserts whatever the person running it happens to be on.
#
# Two measures:
#
#   total  — package versions this binary actually builds (normal edges only:
#            no dev-dependencies, no build scripts, no proc-macro-only crates
#            that vanish at runtime… those are still counted, because they are
#            still compiled).
#   sqlx   — of those, how many are reachable *only* through the sqlx edge.
#            This is the number a swap to rusqlite would actually save, before
#            adding whatever rusqlite brings.
#
# Prints a table and exits 1 if the figures drift from what the notes claim.
set -u
cd "$(dirname "$0")/.." || exit 1

# What concepts/DECISIONS.md D70 and D78 assert, re-measured.
TARGET=x86_64-unknown-linux-musl
WANT_TOTAL=210
WANT_ONLY_SQLX=29

tree=$(mktemp)
trap 'rm -f "$tree"' EXIT
cargo tree --edges normal --prefix depth --no-dedupe --target "$TARGET" >"$tree" 2>/dev/null
[ -s "$tree" ] || { echo "cargo tree failed"; exit 1; }

# The tree goes in by path, not on stdin: the heredoc below is already stdin.
echo "  target                             $TARGET"
python3 - "$WANT_TOTAL" "$WANT_ONLY_SQLX" "$tree" <<'PY'
import re, sys

want_total, want_only = (int(x) for x in sys.argv[1:3])
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

print(f"  package versions actually built    {len(total)}")
print(f"  reachable only through sqlx        {only_sqlx}")
print(f"  without sqlx                       {len(without)}")

bad = 0
for label, got, want in (("total", len(total), want_total),
                         ("only-sqlx", only_sqlx, want_only)):
    if got != want:
        print(f"DRIFT {label}: notes say {want}, the build says {got}")
        bad = 1
sys.exit(bad)
PY
rc=$?
[ $rc -eq 0 ] && echo "deps-count: ok (matches D70/D78)"
exit $rc

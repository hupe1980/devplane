#!/usr/bin/env python3
"""Write the record a full differential run leaves behind.

A run that leaves nothing behind is a claim somebody has to remember. The
measurement is this product's only unoccupied claim, and until this existed the
only trace a full run left was scrollback: whoever ran it knew what it said, and
nobody else could check.

Python rather than `printf` in the shell, and the reason is a bug rather than a
preference: the declared narrowings include `eval "cat .env"`, whose quotes
break a hand-rolled JSON string. Encoding is a job for something that knows how
to encode.

    write-matrix-record.py <path> <vendor> <axis> <cases> <skipped> \\
                           <declared> <reproduced> <verdict> <narrowings-file>

`<narrowings-file>` is one `shape|reason` per line.
"""

import json
import sys
import datetime

(path, vendor, axis, cases, skipped, declared, reproduced, verdict, nfile) = sys.argv[1:10]

narrowings = []
try:
    for line in open(nfile):
        line = line.rstrip("\n")
        if not line:
            continue
        shape, _, reason = line.partition("|")
        narrowings.append({"shape": shape, "reason": reason})
except FileNotFoundError:
    pass

json.dump(
    {
        "run_at": datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        # The version that answered, not the one somebody meant to measure.
        "vendor_version": vendor or None,
        "axis": axis,
        "cases": int(cases),
        # The field that matters most and is the easiest to leave out. A skipped
        # shape is unmeasured, not clean — and the skip set is not stable
        # between runs, because the oracle is a model.
        "skipped": int(skipped),
        "declared_narrowings": int(declared),
        "reproduced": int(reproduced),
        "verdict": verdict,
        "declared": narrowings,
        "measures_only_the_shapes_it_ran": (
            "a skipped shape is unmeasured, not clean; the skip set is not stable "
            "between runs because the oracle is a model, so the honest form of the "
            "claim is 'this run measured these shapes', never 'the matrix is clean'"
        ),
    },
    open(path, "w"),
    indent=2,
)
print(f"verify-permissions-diff: record written to {path}")

#!/usr/bin/env python3
"""Write the record a measurement leaves behind.

Its own file rather than a heredoc inside the driver, for one reason: the record
is what makes *"measured"* survive the question **"since when?"**, so the thing
that writes it has to be checkable without a signed-in agent and real money. A
writer that can only be exercised by paying is a writer nobody exercises.

Called as:

    write-measurement.py <path> <floor> <to> <observed> <verdict> <floor_after>
                         <detail-tsv> <probe-lines>

`<detail-tsv>` is a file of `release\toutcome\tdisposition\tprobe-id\ttext`.
Row text is somebody else's prose: it is read as data, carried as a quoted
report, and never executed or interpolated into a command.
"""

import json, sys, datetime
(path, floor, to, observed, verdict, floor_after, detail_file, probe_lines) = sys.argv[1:9]

# One entry per release, with its rows. Row text is data read from a file and
# never executed; it is somebody else's prose and is carried as a quoted report.
releases, order = {}, []
for line in open(detail_file):
    rel, outcome, disp, pid, text = (line.rstrip("\n").split("\t") + ["", "", "", "", ""])[:5]
    if not rel:
        continue
    if rel not in releases:
        releases[rel] = {"release": rel, "outcome": outcome, "rows": []}
        order.append(rel)
    if disp and disp != "-":
        row = {"disposition": disp, "text": text}
        if pid and pid != "-":
            row["probe_ref"] = pid
        releases[rel]["rows"].append(row)

# One entry per probe, with the outcome the harness reported for it.
probes = []
for line in probe_lines.splitlines():
    parts = line.split()
    if len(parts) >= 3 and parts[0] == "PROBE":
        probes.append({"id": parts[1], "outcome": parts[2],
                       "note": " ".join(parts[3:]).strip("()") or None})

tally = {k: sum(1 for p in probes if p["outcome"] == k)
         for k in ("agreed", "declared", "skipped", "disagreed")}

json.dump({
    "run_at": datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "span": {"from": floor, "to": to},
    "vendor_version": observed,
    "releases": [releases[r] for r in order],
    "probes": probes,
    "tally": tally,
    "verdict": verdict,
    "floor_before": floor,
    "floor_after": floor_after,
    "measures_only_what_was_announced": (
        "this run covered only what these releases announced; it does not "
        "establish that the rest of the matcher still agrees"),
}, open(path, "w"), indent=2)
print(f"measured-through: record written to {path}")

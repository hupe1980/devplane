+++
title = "Conformance"
description = "The permission gate scored against a published execution-boundary profile, including the two properties it does not have."
weight = 25
[extra]
group = "reference"
+++

The gate decides. This page is how much that is worth, and it is written to be checked rather than
believed.

```sh
devplane gate
```

Two halves: how old the measurement is, and what the gate does and does not do.

## The measurement has a date

The permission rules are written in **Claude Code's own syntax**. That is what makes them checkable:
the running product can be asked the same question, call by call, and a disagreement fails the build.
No other gate in this category does that — and no other gate has to admit when the check is stale.

```console
$ devplane gate
measurement
  read          2.1.274 — every rule row the vendor announced up to here has a written disposition
                · does not claim any of it was checked against the running product
  measured      2.1.240 — every rule row up to here produced a probe the running vendor agreed with
                · measured only what the vendor announced; says nothing about the rest of the matcher
  compatibility 2.1.273 — the full differential matrix ran green on both axes
                · does not claim the shapes it runs are complete, nor that skipped shapes are clean
  running       2.1.276 — 3 releases past the compatibility floor
  cadence       full matrix due at 3/10 releases or 1/28 days — not yet owed
```

**Three numbers, because they mean three different things**, and the second line under each is the
half nobody else prints: what it does *not* claim.

**`read`** is the last release whose rule-relevant changelog entries have each been accounted for —
covered by a test, or declined with a reason. It costs one `curl` and a person reading, so it moves
often, and it is a statement about the ledger rather than about the matcher. `just owed` fails when
the vendor has shipped past it; `just advance` moves it, and refuses on a red ledger.

**`measured`** is the last release whose announced rows each produced a probe the running product
agreed with. It costs the probes of one release rather than the whole matrix, so it can move the day
a release ships. It measured only what the vendor announced — the most recent widening this project
found came from a shape no changelog row asked for, so this floor inherits every blind spot those
notes have, and says so.

**`compatibility`** is the last release the full differential matrix ran green against. It costs a
signed-in agent and real money, so it moves rarely. This is the number the central claim rests on.

**`cadence`** is two bounds and never one. The release count tracks drift the cheap run cannot see;
the day count stops a quiet month from letting the expensive clock stop altogether. A single "days
until due" would hide which of the two is the reason.

The gaps between them are the windows a silent divergence lives in. Two of the gate's known defects
were found inside a two-release gap.

**A green run is a claim about the shapes it ran.** The last one put 126 allow cases and 208 deny
cases to Claude Code 2.1.273 and found one real widening, now fixed. Twelve deny shapes were
**skipped rather than measured** — one for a program macOS lacks, eleven because the model answering
the probe does not reliably run them even with nothing forbidden. A skipped shape is unmeasured, not
clean.

Only the [status-line shim](/devplane/docs/observe/#the-status-line) reports a version per session,
so with none installed the command says nothing is reporting — rather than implying there is no gap.

## The card, and its failures

Scored against **EBL-Core** ([arXiv:2609.11596](https://arxiv.org/abs/2609.11596)), a conformance
profile for handing an AI-proposed action to execution authority — published for others to implement
against. A scorecard you write for yourself is a claim.

| Property | | What it means here |
|---|---|---|
| action binding | ✓ | Every decision row names the rule that answered it. A driven agent's request is classified from the protocol, not from its prose. |
| policy non-weakening | ~ | A project cannot widen a machine-wide prohibition, and an agent cannot edit the rules it works under. **But a `git push` inside a script the agent wrote a turn ago is not a tool call, and no hook sees it.** |
| deterministic adjudication | ✓ | A total, synchronous, in-process function with no model and no network in it. The build fails if anything in the core grows a way to wait. |
| derivation verification | ✓ | The rule a verdict names, re-evaluated alone, must produce that verdict. A right answer with a fabricated authority is the one failure an audit log cannot survive. |
| evidence handling | ~ | A gate's exit code, its output and the commit are recorded, and a work's specification is stamped as it was. **Missing: a certificate you can recompute without trusting this tool.** |
| grant lifecycle | ~ | A rule is a standing grant with no expiry and no revocation beyond editing the file. **There is no issuer** — the right answer for a laptop, the wrong one for the profile's intended setting. |
| process separation | ✓ | The gate is a `command` hook: its own process, its own files, no daemon in the path. Any control inside the agent's address space is reachable by inputs that influence it. |
| externalised evidence | ✗ | The decision log is append-only and never pruned, **and it is a table anyone with the file can edit.** Signing and anchoring stay out: one machine, no egress. |

The last two rows are the honest boundary of what a tool-call gate can promise. **A card with nothing
missing on it is a marketing document.**

## What this does not claim

- **It is not a sandbox.** A file a program opens itself is named by no rule, here or in Claude Code.
  The gate is a policy layer; the vendor's sandbox is the boundary, and [security](/devplane/docs/security/)
  says where that line falls.
- **It is not proven.** The gate has been found wrong in the widening direction, silently, more than
  two dozen times. Five mechanisms look for that — the differential harness on both axes, a changelog
  ledger, exhaustive property checks, a test per fix, and using the tool. None is a proof. The gate is
  *demonstrable*, and saying which is which is the point.
- **It is a measurement, so it decays.** That is what the first half of this page is for.

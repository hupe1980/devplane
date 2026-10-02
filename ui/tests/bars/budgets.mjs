// The one copy of every budget the bars enforce, the list of bars that block a
// change, and the phases a surface is exempt from. Each bar reports against its
// own budget; nothing here, or anywhere in the harness, combines them.

/** The bars by number, and the name each is reported under. */
export const NAMES = /** @type {const} */ ({
  1: "first verified change",
  2: "keys to clear the inbox",
  3: "latency",
  4: "no unlabeled state",
});

export const BUDGETS = {
  1: {
    inputs: 12,
    seconds: 120,
    terminal: 0,
    unconfirmedWrites: 0,
    says: "≤ 12 inputs · ≤ 120 s · 0 terminal steps · 0 unconfirmed writes",
  },
  2: {
    keysPerAnswerable: 2,
    says: "≤ 2 keys per answerable item; every item has a keyed way out; no mouse; no offering action",
  },
  3: {
    samples: 30,
    ms: {
      "first paint": 300,
      "surface switch": 100,
      "hunk to hunk": 16,
      "answer to unblocked": 200,
    },
    says: "p95 over ≥ 30 samples: first paint 300 ms · surface switch 100 ms · hunk to hunk 16 ms · answer to unblocked 200 ms",
  },
  4: {
    skeletonMs: 2000,
    says: "0 findings of each kind; a skeleton beside an error for at most 2 s",
  },
};

/// The bars whose failure blocks a change, here and in CI. A bar joins in the
/// change in which it first passes, and only then; 3 never joins (the runner
/// refuses it). A bar not listed is reported in full and marked *not blocking*.
///
/// 2 passes. 4 joins when no surface counts while loading, leaves a skeleton
/// beside an error, or renders absence as zero; 1 joins when the window has a
/// path for every first-run step.
export const BLOCKING = /** @type {number[]} */ ([2]);

/// Surface × phase pairs a bar 4 rule does not apply to, each with its reason,
/// printed every run. The surface is a registered id; `rule` is one finding
/// kind, or `*` for every rule in that phase.
/** @type {{ surface: string, phase: string, rule: string, reason: string }[]} */
export const EXEMPT = [
  { surface: "change", phase: "nothing visible", rule: "absence-equals-zero", reason: "lists the changes Devplane started, which it sees whether or not any session channel reports" },
  { surface: "plan", phase: "nothing visible", rule: "absence-equals-zero", reason: "reads specifications from disk, not from sight of sessions" },
  { surface: "github", phase: "nothing visible", rule: "absence-equals-zero", reason: "reads the forge, not sight of sessions" },
  { surface: "quit", phase: "nothing visible", rule: "absence-equals-zero", reason: "names the agents Devplane started, which it sees whether or not any session channel reports" },
];

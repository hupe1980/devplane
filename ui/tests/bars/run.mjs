// The bars: the interface's felt quality, measured on the tool over recorded
// fixtures (and, for the first verified change, a real host and the echo agent).
//
//   npm run bars                 # bars 1, 2 and 4
//   npm run bars -- --bar 4      # one bar (repeatable)
//   npm run bars -- --bar 3      # latency: reported, never blocking
//
// Exit 0 when every blocking bar that ran passed; 1 when one failed; 2 when the
// harness could not start. A failing bar that is not blocking is printed in
// full and does not change the exit status.

import { BLOCKING, BUDGETS } from "./budgets.mjs";
import { HarnessError, start } from "./driver.mjs";
import { print } from "./report.mjs";

/** @type {Record<number, () => Promise<{ default: (driver: any, budget: any) => Promise<{ measured: string[], findings: string[], notes?: string[] }> }>>} */
const BARS = {
  1: () => import("./bar1.mjs"),
  2: () => import("./bar2.mjs"),
  3: () => import("./bar3.mjs"),
  4: () => import("./bar4.mjs"),
};

function args() {
  /** @type {number[]} */
  const want = [];
  const a = process.argv.slice(2);
  for (let i = 0; i < a.length; i++) {
    if (a[i] === "--bar") {
      const n = Number(a[++i]);
      if (!BARS[n]) throw new HarnessError(`no bar ${a[i]}: the bars are 1, 2, 3 and 4`);
      if (!want.includes(n)) want.push(n);
    } else {
      throw new HarnessError(`unknown argument ${a[i]} — use --bar N`);
    }
  }
  // Cheapest first; latency only when asked for.
  return want.length ? want : [4, 2, 1];
}

/** @type {any} */
let driver = null;
let status = 0;
try {
  if (BLOCKING.includes(3)) throw new HarnessError("bar 3 is on the blocking list; latency reports and never blocks a change");
  const which = args();
  // Bar 1 drives its own host; the others share the fixture server and browser.
  driver = await start();
  const bars = [];
  for (const n of which) {
    const blocking = BLOCKING.includes(n);
    /** @type {any} */
    const budget = BUDGETS[/** @type {1|2|3|4} */ (n)];
    try {
      const run = (await BARS[n]()).default;
      const { measured, findings, notes } = await run(driver, budget);
      bars.push({ n, budget: budget.says, measured, findings, notes, result: findings.length ? "failed" : "passed", blocking });
    } catch (e) {
      if (e instanceof HarnessError) throw e;
      const said = e instanceof Error ? e.message.split("\n")[0] : String(e);
      bars.push({ n, budget: budget.says, measured: [], findings: [`the bar stopped: ${said}`], result: "failed", blocking });
    }
  }
  print(/** @type {any} */ (bars));
  status = bars.some((b) => b.blocking && b.result === "failed") ? 1 : 0;
} catch (e) {
  console.error(`bars: ${e instanceof Error ? e.message : String(e)}`);
  status = 2;
} finally {
  await driver?.close();
}
process.exit(status);

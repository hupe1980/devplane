// The report: one section per bar run, each with its budget, what it measured,
// every finding, and *passed* or *failed* in words — *not blocking* where the
// bar is not on the blocking list. Then the machine class. No line follows the
// sections: there is no total, grade or combined figure.
//
// Nothing identifies a person: the machine is its platform, architecture, CPU
// model and core count — no host name, user name or path.

import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import os from "node:os";
import { here } from "./driver.mjs";
import { NAMES } from "./budgets.mjs";

/**
 * @typedef {{ n: 1 | 2 | 3 | 4, budget: string, measured: string[], findings: string[], notes?: string[], result: "passed" | "failed", blocking: boolean }} Bar
 */

export function machine() {
  const cpus = os.cpus();
  return `${os.platform()} ${os.arch()} · ${cpus[0]?.model?.trim() ?? "unknown CPU"} · ${cpus.length} cores`;
}

/** @param {Bar[]} bars */
export function print(bars) {
  const lines = [];
  for (const b of bars) {
    const head = `bar ${b.n} — ${NAMES[b.n]}`;
    const verdict = b.result === "passed" ? "passed" : b.blocking ? "failed" : "failed — not blocking";
    lines.push(`${head.padEnd(52)} ${verdict}${b.result === "passed" && !b.blocking ? " — not blocking" : ""}`);
    lines.push(`  budget   ${b.budget}`);
    for (const m of b.measured) lines.push(`  measured ${m}`);
    for (const f of b.findings) lines.push(`  finding  ${f}`);
    for (const n of b.notes ?? []) lines.push(`  ${n}`);
    lines.push("");
  }
  lines.push(`machine    ${machine()}`);
  const text = lines.join("\n");
  console.log(text);
  const dir = resolve(here, ".bars");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    resolve(dir, "report.json"),
    JSON.stringify(
      {
        machine: machine(),
        bars: bars.map((b) => ({ n: b.n, name: NAMES[b.n], budget: b.budget, measured: b.measured, result: b.result, blocking: b.blocking, findings: b.findings, notes: b.notes ?? [] })),
      },
      null,
      2,
    ),
  );
  return text;
}

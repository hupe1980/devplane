// Bar 3 — latency. The 95th percentile (nearest rank) of at least 30 samples
// of four interactions, each against its own budget; fewer samples read *not
// measured*. Reported, never blocking, and never retried until it passes.
//
//   first paint          navigation start → the landing surface shows loaded
//                        data, laid out, with no skeleton
//   surface switch       the address set → the new surface's heading is the
//                        page's and its content is laid out
//   hunk to hunk         the keydown on `j` in the review of the forty-file
//                        change → the next hunk laid out
//   answer to unblocked  the answer's commit (the host's response to it; no
//                        hold delays an answer yet) → the echo agent's own
//                        message saying it received the answer, read from a
//                        real host
//
// Times are taken in the page with `performance.now()` around the interface's
// own work: a keydown listener the harness installs runs first, and "laid out"
// is a forced layout after every queued update has run. They are properties
// of the tool on a scripted fixture; nothing here times a person.

import { spawn, execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { here, recorded, registry, sleep } from "./driver.mjs";

/** Nearest-rank p95. @param {number[]} xs */
const p95 = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.max(0, Math.ceil(0.95 * s.length) - 1)];
};

/** In the page: resolves once every queued update has run and layout is forced. */
const SETTLED = `() => new Promise((r) => setTimeout(() => { document.body.getBoundingClientRect(); r(performance.now()); }, 0))`;

/** @param {any} driver @param {number} n */
async function firstPaint(driver, n) {
  /** @type {number[]} */
  const out = [];
  for (let i = 0; i < n; i++) {
    const page = await driver.page();
    try {
      await page.goto(`${driver.base}#inbox`);
      const t = await page.evaluate(`(async () => {
        const ready = () => {
          const main = document.querySelector("main#surface");
          if (!main || !main.innerText.trim()) return false;
          if (document.querySelector("main#surface [aria-busy=true], aside.side [aria-busy=true]")) return false;
          return !!document.querySelector("aside.side [role=option], main#surface h1, main#surface h2");
        };
        while (!ready()) await new Promise((r) => setTimeout(r, 1));
        return (${SETTLED})();
      })()`);
      out.push(t);
    } finally {
      await page.context().close();
    }
  }
  return out;
}

/** @param {any} driver @param {number} n */
async function surfaceSwitch(driver, n) {
  /** @type {number[]} */
  const out = [];
  const page = await driver.page();
  try {
    await page.goto(`${driver.base}#inbox`);
    await page.locator("main#surface").waitFor({ timeout: 10_000 });
    await sleep(1000);
    const surfaces = (await registry(page)).filter((s) => !s.transient && !s.bare);
    if (!surfaces.length) return out;
    // A first round loads every surface's code: what is timed is the switch.
    for (const s of surfaces) {
      await page.evaluate((/** @type {string} */ h) => (location.hash = h), `#${s.id}`);
      await sleep(300);
    }
    for (let i = 0; out.length < n && i < n * 2; i++) {
      const s = surfaces[i % surfaces.length];
      const t = await page.evaluate(
        `(async ([hash, heading]) => {
          const t0 = performance.now();
          location.hash = hash;
          const ready = () => {
            const main = document.querySelector("main#surface");
            return !!main && main.getAttribute("aria-label") === heading && main.innerText.trim().length > 0;
          };
          while (!ready()) await new Promise((r) => setTimeout(r, 0));
          return (${SETTLED})().then((t1) => t1 - t0);
        })(${JSON.stringify([`#${s.id}`, s.heading])})`,
      );
      out.push(t);
      await sleep(50);
    }
  } finally {
    await page.context().close();
  }
  return out;
}

/** @param {any} driver @param {number} n */
async function hunkToHunk(driver, n) {
  /** @type {number[]} */
  const out = [];
  const forty = recorded("change", "Forty files");
  if (!forty) return out;
  const page = await driver.page();
  try {
    await page.addInitScript(`
      window.__hunks = [];
      addEventListener("keydown", (e) => {
        if (e.key !== "j") return;
        const t0 = performance.now();
        let moved = false;
        const watch = new MutationObserver(() => (moved = true));
        watch.observe(document.querySelector("main#surface") ?? document.body, { subtree: true, childList: true, attributes: true, characterData: true });
        (${SETTLED})().then((t1) => { watch.disconnect(); window.__hunks.push({ ms: t1 - t0, moved }); });
      }, { capture: true });
    `);
    await page.goto(`${driver.base}#change/${forty.id}/review`);
    await page.locator("main#surface [id^=hunk-]").first().waitFor({ timeout: 10_000 });
    await sleep(800);
    await page.evaluate(() => /** @type {HTMLElement | null} */ (document.activeElement)?.blur());
    for (let i = 0; i < n + 5; i++) {
      await page.keyboard.press("j");
      await sleep(60);
    }
    await sleep(300);
    const got = await page.evaluate(() => /** @type {any} */ (window).__hunks);
    // A press that moved nothing (the last hunk) is not a sample.
    for (const g of got) if (g.moved) out.push(g.ms);
  } finally {
    await page.context().close();
  }
  return out;
}

/** @param {number} n */
async function answerToUnblocked(n) {
  /** @type {number[]} */
  const out = [];
  const root = resolve(here, "..");
  const bin = process.env.DEVPLANE_BIN ?? resolve(root, "target/debug/devplane");
  const echo = process.env.ECHO_AGENT ?? resolve(bin, "../examples/echo_agent");
  if (!existsSync(bin) || !existsSync(echo)) return { out, why: `no host binary or echo agent at ${bin}` };
  const sb = mkdtempSync(join(tmpdir(), "devplane-bar3-"));
  const home = join(sb, "home");
  const repo = join(sb, "sample");
  mkdirSync(home, { recursive: true });
  mkdirSync(join(sb, "claude", "projects"), { recursive: true });
  mkdirSync(repo, { recursive: true });
  const genv = { ...process.env, GIT_AUTHOR_NAME: "Bar", GIT_AUTHOR_EMAIL: "bar@example.com", GIT_COMMITTER_NAME: "Bar", GIT_COMMITTER_EMAIL: "bar@example.com" };
  execFileSync("git", ["init", "-q", "-b", "main"], { cwd: repo, env: genv });
  writeFileSync(join(repo, ".gitignore"), ".devplane/\n");
  writeFileSync(join(repo, "README.md"), "# sample\n");
  execFileSync("git", ["add", "-A"], { cwd: repo, env: genv });
  execFileSync("git", ["commit", "-qm", "init"], { cwd: repo, env: genv });
  /** @type {Record<string, string>} */
  const env = {};
  for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !/^(CLAUDECODE|CLAUDE_CODE_|DEVPLANE_RUN|CODEX_|COPILOT_CLI)/.test(k)) env[k] = v;
  Object.assign(env, { DEVPLANE_HOME: home, CLAUDE_CONFIG_DIR: join(sb, "claude") });
  const host = spawn(bin, ["up", "--no-browser"], { cwd: sb, env, stdio: "ignore" });
  try {
    for (let i = 0; i < 100 && !existsSync(join(home, "host.json")); i++) await sleep(100);
    if (!existsSync(join(home, "host.json"))) return { out, why: "the host did not start" };
    await sleep(200);
    execFileSync(bin, ["project", "trust", repo, "--yes"], { env, stdio: "pipe" });
    const port = JSON.parse(readFileSync(join(home, "host.json"), "utf8")).port;
    const token = readFileSync(join(home, "token"), "utf8").trim();
    const api = async (/** @type {string} */ path, /** @type {any} */ body) => {
      const r = await fetch(`http://127.0.0.1:${port}${path}`, {
        method: body === undefined ? "GET" : "POST",
        headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
      if (!r.ok) throw new Error(`${path}: ${r.status} ${await r.text()}`);
      return r.json();
    };
    /** @type {Set<string>} */
    const asked = new Set();
    for (let i = 0; i < n; i++) {
      const c = await api("/api/changes", { cwd: repo, title: `Ask ${i}`, agent: echo, prompt: "question" });
      const changeId = c.id ?? c.change?.id;
      // The question, as the inbox raises it.
      /** @type {any} */
      let item = null;
      for (let w = 0; w < 200 && !item; w++) {
        const inbox = await api("/api/inbox");
        // The new question: the one not answered yet (a question names its run, not its change).
        item = (inbox.items ?? []).find((/** @type {any} */ x) => x.kind === "question" && x.ask && !asked.has(x.ask));
        if (!item) await sleep(50);
      }
      // A question that never arrived is a missing sample, not a slow one.
      if (!item) continue;
      asked.add(item.ask);
      const run = item.run_id;
      const said = async () => {
        const m = await api(`/api/runs/${encodeURIComponent(run)}/messages`);
        return JSON.stringify(m).includes("answer:");
      };
      const option = item.options?.[0]?.id ?? item.form?.[0]?.options?.[0]?.value;
      const body = item.form?.length ? { option, field: item.form[0].field } : { option };
      await api(`/api/asks/${encodeURIComponent(item.ask)}/answer`, body);
      const t0 = performance.now();
      let got = false;
      while (performance.now() - t0 < 5000) {
        if (await said()) {
          got = true;
          break;
        }
      }
      if (got) out.push(performance.now() - t0);
    }
    return { out, why: "" };
  } finally {
    host.kill();
    await sleep(300);
    rmSync(sb, { recursive: true, force: true });
  }
}

/** @param {any} driver @param {any} budget */
export default async function bar3(driver, budget) {
  /** @type {string[]} */
  const measured = [];
  /** @type {string[]} */
  const findings = [];
  const n = budget.samples;
  const answered = await answerToUnblocked(n).catch((e) => ({ out: [], why: e instanceof Error ? e.message.split("\n")[0] : String(e) }));
  /** @type {[string, number[], string][]} */
  const rows = [
    ["first paint", await firstPaint(driver, n), ""],
    ["surface switch", await surfaceSwitch(driver, n), ""],
    ["hunk to hunk", await hunkToHunk(driver, n), recorded("change", "Forty files") ? "" : "no recorded change titled Forty files"],
    ["answer to unblocked", answered.out, answered.why],
  ];
  for (const [name, xs, why] of rows) {
    const limit = budget.ms[name];
    if (xs.length < n) {
      measured.push(`${name.padEnd(20)} not measured — ${xs.length} of ${n} samples${why ? ` (${why})` : ""} · budget ${limit} ms`);
      findings.push(`${name} · not measured · ${xs.length} samples`);
      continue;
    }
    const p = p95(xs);
    measured.push(`${name.padEnd(20)} p95 ${p.toFixed(1)} ms over ${xs.length} samples · budget ${limit} ms${p > limit ? " · over" : ""}`);
    if (p > limit) findings.push(`${name} · p95 ${p.toFixed(1)} ms over ${xs.length} samples, budget ${limit} ms`);
  }
  return { measured, findings };
}

// The interface in a real browser, over the recorded fixtures: the bugs a
// server render cannot see (a poll that resets a tab, a reply that carries to
// the next item, Enter on a focused button, a ⌥ chord on a Mac, a failure
// drawn as calm, a long write cut off). Playwright is not a dependency of this
// package; this runs where it is installed and says so where it is not.
//
//   PLAYWRIGHT=/path/to/node_modules/playwright-core node tests/browser.mjs
//
// `CHROME` names a browser binary (default: the system Chrome on macOS, else
// Playwright's own). It starts `vite` with `DEVPLANE_FIXTURES=1` on a free
// port, or uses `BASE` (e.g. http://localhost:5290/) if one is running.
// `npm run render` stays the CI check; this is the one a person runs.

import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { resolve } from "node:path";

const here = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const where = process.env.PLAYWRIGHT;
if (!where) {
  console.log("browser: skipped — set PLAYWRIGHT to a playwright-core install to run it");
  process.exit(0);
}
/** @type {any} */
let pw;
try {
  pw = await import(existsSync(where) ? pathToFileURL(resolve(where, existsSync(resolve(where, "index.mjs")) ? "index.mjs" : "index.js")).href : where);
} catch (e) {
  console.log(`browser: skipped — ${where} could not be loaded (${e instanceof Error ? e.message : String(e)})`);
  process.exit(0);
}
const chromium = pw.chromium ?? pw.default?.chromium;

/** @type {{ close(): Promise<void> } | null} */
let server = null;
let BASE = process.env.BASE ?? "";
if (!BASE) {
  process.env.DEVPLANE_FIXTURES = "1";
  const { createServer } = await import("vite");
  const s = await createServer({ root: here, configFile: resolve(here, "vite.config.ts"), logLevel: "error", server: { port: 0, host: "127.0.0.1" } });
  await s.listen();
  server = s;
  BASE = s.resolvedUrls?.local[0] ?? "";
}
const mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const executablePath = process.env.CHROME ?? (existsSync(mac) ? mac : undefined);
const browser = await chromium.launch({ executablePath, headless: true });
const sleep = (/** @type {number} */ ms) => new Promise((r) => setTimeout(r, ms));
const CHANGE = "c-01a0dbe60c9b76e6936cf1fc4701a22f";

let failures = 0;
/** @param {string} name @param {boolean} ok @param {string} [detail] */
function check(name, ok, detail = "") {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? ` — ${detail}` : ""}`);
  if (!ok) failures += 1;
}
async function page() {
  const c = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const p = await c.newPage();
  p.on("pageerror", (/** @type {Error} */ e) => check(`no page error`, false, e.message));
  return p;
}
const selectedTab = async (/** @type {any} */ p) =>
  ((await p.locator(".doc [role=tab][aria-selected=true]").allTextContents()).join("")).replace(/\s+/g, " ").trim();

try {
  // 1. The change page keeps its tab and its result through the poll.
  {
    const p = await page();
    await p.goto(`${BASE}#change/${CHANGE}`);
    await p.locator(".doc").waitFor();
    await p.getByRole("button", { name: /Offer as pull request/ }).click();
    await sleep(500);
    check("an offer's refusal is shown", (await p.locator(".refusal").count()) === 1);
    let reads = 0;
    p.on("request", (/** @type {any} */ r) => {
      if (new URL(r.url()).pathname === `/api/changes/${CHANGE}`) reads += 1;
    });
    await sleep(5000);
    check("the refusal is still shown after 5 s", (await p.locator(".refusal").count()) === 1);
    check("the change is not re-read while nothing moved", reads === 0, `${reads} reads in 5 s`);
    await p.locator(".doc [role=tab]", { hasText: "Gates" }).click();
    await sleep(5000);
    check("a picked tab stays picked after 5 s", (await selectedTab(p)).startsWith("Gates"), await selectedTab(p));
    await p.context().close();
  }

  // 2. A reply typed for one item never carries to the next.
  {
    const p = await page();
    await p.route("**/api/inbox*", (/** @type {any} */ r) => {
      const q = (/** @type {string} */ id, /** @type {string} */ t) => ({ id, kind: "question", level: "high", title: t, ask: id, actions: ["reply", "open"], options: [] });
      return r.fulfill({ json: { items: [q("ask-A", "Question A"), q("ask-B", "Question B")], folded: [], inhibited: [], close: null } });
    });
    await p.goto(`${BASE}#inbox/item=ask-A`);
    await p.getByLabel("your answer to: Question A").fill("yes, delete the prod table");
    await p.evaluate(() => (location.hash = "#inbox/item=ask-B"));
    await sleep(500);
    const v = await p.getByLabel("your answer to: Question B").inputValue();
    check("a typed reply does not carry to the next item", v === "", JSON.stringify(v));
    // j with focus on the page moves to the next item; k back.
    await p.evaluate(() => (location.hash = "#inbox/item=ask-A"));
    await sleep(300);
    await p.evaluate(() => /** @type {HTMLElement | null} */ (document.activeElement)?.blur());
    await p.keyboard.press("j");
    await sleep(300);
    check("j on the page moves the inbox to the next item", /^#inbox\/item(=|%3D)ask-B$/.test(await p.evaluate(() => location.hash)));
    await p.context().close();
  }

  // 3. Enter presses a focused button; ⌥N (as macOS sends it) opens New change.
  {
    const p = await page();
    /** @type {string[]} */
    const posts = [];
    p.on("request", (/** @type {any} */ r) => {
      if (r.method() === "POST") posts.push(r.url());
    });
    await p.goto(`${BASE}#inbox/${encodeURIComponent(`item=${CHANGE}:ready_to_decide`)}`);
    const snooze = p.getByRole("button", { name: /snooze/ }).first();
    await snooze.waitFor();
    await snooze.focus();
    await p.keyboard.press("Enter");
    await sleep(500);
    check("Enter on a focused inbox button presses it", posts.some((u) => u.includes("/snooze")), `${posts.length} POSTs`);
    await p.evaluate(() => {
      /** @type {HTMLElement | null} */ (document.activeElement)?.blur();
      dispatchEvent(new KeyboardEvent("keydown", { key: "Dead", code: "KeyN", altKey: true, bubbles: true }));
    });
    await sleep(300);
    check("⌥N as macOS sends it (e.key Dead) opens New change", (await p.evaluate(() => location.hash)) === "#new");
    await p.context().close();
  }

  // 4. Search keeps what is typed; one poll however much you move; the skip link keeps the address.
  {
    const p = await page();
    await p.route("**/api/search*", (/** @type {any} */ r) => r.fulfill({ json: { hits: [] } }));
    await p.goto(`${BASE}#search/foo`);
    const box = p.locator("input[type=search]");
    await box.fill("something else");
    await sleep(2600);
    check("the search box keeps what was typed through a poll", (await box.inputValue()) === "something else");
    let boards = 0;
    p.on("request", (/** @type {any} */ r) => {
      if (r.url().includes("/api/board")) boards += 1;
    });
    for (const h of ["#change", "#board", "#plan", "#inbox", "#setup", "#change", "#board", "#inbox", "#reports", "#board"]) {
      await p.evaluate((/** @type {string} */ h) => (location.hash = h), h);
      await sleep(300);
    }
    check("moving between surfaces does not restart the poll", boards <= 3, `${boards} board reads in 3 s over 10 moves`);
    await p.goto(`${BASE}#change/${CHANGE}`);
    await p.locator(".doc").waitFor();
    await p.keyboard.press("Tab");
    await p.keyboard.press("Enter");
    await sleep(200);
    check("the skip link keeps the address", (await p.evaluate(() => location.hash)) === `#change/${CHANGE}`);
    await p.context().close();
  }

  // 5. A failed read is said, never drawn as calm or empty.
  {
    const p = await page();
    await p.route("**/api/projects", (/** @type {any} */ r) => r.fulfill({ status: 500, json: { error: "database is locked" } }));
    await p.goto(`${BASE}#new`);
    await sleep(1200);
    const t = await p.locator("form.new").innerText();
    check("New change says the projects could not be read", /Could not read the projects: database is locked/.test(t) && !/No project is registered/.test(t));
    await p.context().close();
  }
  {
    const p = await page();
    await p.route("**/api/inbox?needs_you=true", (/** @type {any} */ r) => r.fulfill({ status: 500, json: { error: "database is locked" } }));
    await p.goto(`${BASE}#answer`);
    await sleep(1500);
    const t = await p.locator("main").innerText();
    check("the answer window says the read failed, not that nothing needs you", /Could not read/.test(t) && !/Nothing needs you/.test(t));
    await p.context().close();
  }
  {
    const p = await page();
    await p.route("**/messages*", (/** @type {any} */ r) => r.fulfill({ status: 500, json: { error: "database is locked" } }));
    await p.goto(`${BASE}#change/${CHANGE}`);
    await p.locator(".doc").waitFor();
    await p.locator(".doc [role=tab]", { hasText: "Agent" }).click();
    await sleep(800);
    const t = await p.locator(".agent").innerText();
    check("the agent view says the conversation was not read", /Could not read the conversation/.test(t) && !/Nothing has been said yet/.test(t));
    await p.context().close();
  }

  // 6. A watched run is read again; a long write is not cut off, and says what it came to.
  {
    const p = await page();
    let said = 0;
    p.on("request", (/** @type {any} */ r) => {
      if (r.url().includes("/messages")) said += 1;
    });
    await p.route(`**/api/changes/${CHANGE}/verify`, async (/** @type {any} */ r) => {
      await sleep(12_000);
      await r.fulfill({ json: { passed: false, summary: "check failed: 1 of 3 commands" } });
    });
    await p.route(`**/api/changes/${CHANGE}/offer`, (/** @type {any} */ r) =>
      r.fulfill({ json: { offer: "commands", push: "git push -u origin change/x", create: "gh pr create --head change/x" } }),
    );
    await p.goto(`${BASE}#change/${CHANGE}`);
    await p.locator(".doc").waitFor();
    await p.locator(".doc [role=tab]", { hasText: "Agent" }).click();
    await sleep(4500);
    check("the agent view reads a watched run again", said >= 2, `${said} reads in 4.5 s`);
    await p.getByRole("button", { name: /Run gates/ }).click();
    await sleep(11_000);
    const during = await p.locator(".said").first().innerText();
    check("a long gate run shows its elapsed time, not a timeout", /Running the gates · 1[01]s/.test(during), during);
    check("a second run cannot be started meanwhile", await p.getByRole("button", { name: /Run gates/ }).isDisabled());
    await sleep(2000);
    const after = await p.locator(".said").first().innerText();
    check("the finished run says what it came to", /The gates failed: check failed/.test(after) && !/running/.test(after), after);
    await p.getByRole("button", { name: /Offer as pull request/ }).click();
    await sleep(500);
    const offered = await p.locator(".head").innerText();
    check("an offer that handed back commands shows them", offered.includes("git push -u origin change/x") && offered.includes("gh pr create"));
    check("…and does not say it was offered", !/offer made/.test(offered));
    await p.context().close();
  }
} catch (e) {
  check("the check ran to the end", false, e instanceof Error ? e.message.split("\n")[0] : String(e));
} finally {
  await browser.close();
  await server?.close();
}

if (failures) {
  console.error(`browser: ${failures} failure(s)`);
  process.exit(1);
}
console.log("browser: ok");

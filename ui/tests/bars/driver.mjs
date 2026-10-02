// What every bar and the browser checks share: Playwright, the fixture server,
// Chromium, the interface's own surface registry, and the recorded subjects.
//
// `PLAYWRIGHT` names a playwright-core install (else `ui/node_modules`); a
// missing one is a harness error, never a pass. `CHROME` names a browser binary
// (default: the system Chrome on macOS, else Playwright's own Chromium). `BASE`
// names a running `npm run dev:fixtures`; otherwise Vite is started with
// `DEVPLANE_FIXTURES=1` on a free loopback port.

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { resolve } from "node:path";

/** The `ui/` directory. */
export const here = resolve(fileURLToPath(new URL(".", import.meta.url)), "../..");
export const fixtures = resolve(here, "fixtures");

/// The harness could not start: no Playwright, no browser, a bad argument.
/// Exit 2, distinct from a bar that failed.
export class HarnessError extends Error {}

export const sleep = (/** @type {number} */ ms) => new Promise((r) => setTimeout(r, ms));

/** @returns {Promise<any>} the module exporting `chromium` */
export async function playwright() {
  const where = process.env.PLAYWRIGHT;
  const candidates = where ? [where] : [resolve(here, "node_modules/playwright-core"), "playwright-core"];
  /** @type {string[]} */
  const why = [];
  for (const c of candidates) {
    try {
      const href = existsSync(c)
        ? pathToFileURL(resolve(c, existsSync(resolve(c, "index.mjs")) ? "index.mjs" : "index.js")).href
        : c;
      const pw = await import(href);
      const chromium = pw.chromium ?? pw.default?.chromium;
      if (chromium) return chromium;
      why.push(`${c}: no chromium export`);
    } catch (e) {
      why.push(`${c}: ${e instanceof Error ? e.message.split("\n")[0] : String(e)}`);
    }
  }
  throw new HarnessError(
    `no playwright-core to drive a browser (${why.join("; ")}) — install one ` +
      "(`npm install --no-save playwright-core@1.62.1` in ui/, then `npx playwright-core install chromium`) or set PLAYWRIGHT",
  );
}

/**
 * Starts the fixture server (unless `BASE` is set) and a browser.
 * @returns {Promise<Driver>}
 */
export async function start() {
  const chromium = await playwright();
  /** @type {{ close(): Promise<void> } | null} */
  let server = null;
  let base = process.env.BASE ?? "";
  if (!base) {
    process.env.DEVPLANE_FIXTURES = "1";
    const { createServer } = await import("vite");
    const s = await createServer({
      root: here,
      configFile: resolve(here, "vite.config.ts"),
      logLevel: "error",
      // No hot reload: a file saved mid-run must not swap code under a page.
      server: { port: 0, host: "127.0.0.1", hmr: false },
    });
    await s.listen();
    server = s;
    base = s.resolvedUrls?.local[0] ?? "";
  }
  if (!base.endsWith("/")) base += "/";
  const mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
  const executablePath = process.env.CHROME ?? (process.platform === "darwin" && existsSync(mac) ? mac : undefined);
  /** @type {any} */
  let browser;
  try {
    browser = await chromium.launch({ executablePath, headless: true });
  } catch (e) {
    await server?.close();
    throw new HarnessError(`no browser could be launched: ${e instanceof Error ? e.message.split("\n")[0] : String(e)} — \`npx playwright-core install chromium\`, or set CHROME`);
  }
  return {
    base,
    browser,
    /** A fresh context at 1280×800, so no state carries between pages. */
    async page(/** @type {{ width?: number, height?: number }} */ size = {}) {
      const c = await browser.newContext({ viewport: { width: size.width ?? 1280, height: size.height ?? 800 } });
      return c.newPage();
    },
    async close() {
      await browser.close().catch(() => {});
      await server?.close();
    },
  };
}

/**
 * @typedef {{ base: string, browser: any, page(size?: { width?: number, height?: number }): Promise<any>, close(): Promise<void> }} Driver
 * @typedef {{ id: string, title: string, heading: string, link: string | null, holds: string | null, bare: boolean, transient: boolean, nav: boolean, reads: string[] }} Registered
 */

/**
 * The surfaces the interface registers, read from its own registry in the
 * running page (the module the app itself imported), so a new surface is
 * visited without editing a bar. Zero is returned as zero: the bars fail on it.
 * @param {any} page a page already at the app
 * @returns {Promise<Registered[]>}
 */
export async function registry(page) {
  return page.evaluate(async () => {
    const m = await import(/* @vite-ignore */ "/src/lib/surfaces.ts");
    return m.surfaces().map((/** @type {any} */ s) => ({
      id: s.id,
      title: s.title,
      heading: s.heading,
      link: s.link ?? null,
      holds: s.holds ?? null,
      bare: !!s.bare,
      transient: !!s.transient,
      nav: s.nav !== false,
      reads: s.reads ?? [],
    }));
  });
}

/** Every recording at the top of `ui/fixtures/`, by file name. */
export function recordings() {
  /** @type {Record<string, any>} */
  const out = {};
  for (const f of readdirSync(fixtures)) {
    if (f.endsWith(".json")) out[f.slice(0, -5)] = JSON.parse(readFileSync(resolve(fixtures, f), "utf8"));
  }
  return out;
}

/**
 * A recorded change (or review) found by its title, as the render suite finds
 * it: file names carry ids that change on every capture.
 * @param {"change" | "review"} kind
 * @param {string} title
 */
export function recorded(kind, title) {
  for (const [name, body] of Object.entries(recordings())) {
    if (name.startsWith(`${kind}-`) && body?.title === title) return body;
  }
  return null;
}

/** The weakened change every bar reads its subject from. */
export const SUBJECT = "Rate-limit the login route";

/**
 * The recorded subject a surface opened about a kind of object is opened on:
 * the weakened change, the first ask in the inbox, the first recorded run.
 * @param {string} kind `change`, `ask` or `run`
 */
export function subject(kind) {
  const all = recordings();
  if (kind === "change") return recorded("change", SUBJECT)?.id ?? null;
  if (kind === "ask") return (all.inbox?.items ?? []).find((/** @type {any} */ i) => i.ask)?.ask ?? null;
  if (kind === "run") {
    const run = Object.keys(all).find((n) => n.startsWith("run-"));
    return run ? run.slice(4) : null;
  }
  return null;
}

/** The phases bar 4 forces, in the order it visits them. */
export const PHASES = /** @type {const} */ (["loading", "empty", "nothing visible", "failed", "populated"]);

/** The planted error text in the *failed* phase. */
export const PLANTED = "bars: planted failure";

/**
 * Answers `/api/**` for one phase, from the page (research R4):
 * *loading* holds every read; *empty* and *nothing visible* answer from
 * `ui/fixtures/phases/<name>/`, falling back to the populated recording;
 * *failed* answers every read 500 with a planted error, and the forge 200 with
 * its recording plus `not signed in to GitHub`; *populated* is unrouted.
 * @param {any} page
 * @param {typeof PHASES[number]} name
 * @returns {Promise<Set<string>>} the routes the phase overrode, as they are read
 */
export async function phase(page, name) {
  /** @type {Set<string>} */
  const overrode = new Set();
  if (name === "populated") return overrode;
  if (name === "loading") {
    await page.route("**/api/**", () => {
      /* held: never answered while the surface is read */
    });
    return overrode;
  }
  if (name === "failed") {
    const forge = recordings().forge ?? {};
    await page.route("**/api/**", (/** @type {any} */ r) => {
      const path = new URL(r.request().url()).pathname;
      if (r.request().method() !== "GET") return r.fulfill({ json: {} });
      overrode.add(path);
      if (path === "/api/forge") return r.fulfill({ json: { ...forge, error: "not signed in to GitHub" } });
      return r.fulfill({ status: 500, json: { error: `${PLANTED} ${path}` } });
    });
    return overrode;
  }
  const dir = resolve(fixtures, "phases", name.replace(/ /g, "-"));
  await page.route("**/api/**", (/** @type {any} */ r) => {
    const path = new URL(r.request().url()).pathname;
    const parts = path.slice(5).split("/");
    if (r.request().method() !== "GET" || parts.length !== 1) return r.fallback();
    const file = resolve(dir, `${parts[0]}.json`);
    if (!existsSync(file)) return r.fallback();
    overrode.add(path);
    return r.fulfill({ contentType: "application/json", body: readFileSync(file, "utf8") });
  });
  return overrode;
}

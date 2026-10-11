// Bar 2 — keys to clear the inbox. Every item in the recorded inbox is taken
// out by key presses alone, and the presses are counted: at most two for an
// item Devplane can answer (a question it holds, a permission it holds); at
// least one keyed way out for every other item; never the mouse.
//
// The guard: no action reachable from an inbox item — its buttons and links,
// the keys bound in the inbox, the palette opened over it — offers a change,
// by its label or by the request it sends.
//
// Writes change what the inbox reads within the bar's own pages: an answered
// or snoozed item leaves the next read, as it leaves the host's. The shared
// fixture server stays as recorded.

import { recordings, registry, sleep } from "./driver.mjs";

const OFFER = /offer/i;

/** @param {any} i */
const answerable = (i) =>
  (i.kind === "question" && !!i.ask && ((i.options ?? []).length > 0 || (i.form ?? []).length > 0 || (i.actions ?? []).includes("reply"))) ||
  (i.kind === "permission" && !!(i.ask ?? i.request_id) && (i.actions ?? []).some((/** @type {string} */ a) => a === "allow" || a === "deny"));

/**
 * The inbox as the host would answer it after the writes the bar made: an
 * answer or a snooze takes its item out; `minutes=0` puts a snooze back.
 * @param {any} inbox the recording
 */
function host(inbox) {
  /** @type {Set<string>} */
  const gone = new Set();
  /** @type {string[]} */
  const offers = [];
  /** @type {string[]} */
  const writes = [];
  /** @param {string} path @param {string} query */
  function write(path, query) {
    const items = inbox.items ?? [];
    let m;
    /** @type {any[]} */
    let hit = [];
    if ((m = /^\/api\/asks\/([^/]+)\/answer$/.exec(path))) {
      const id = decodeURIComponent(m[1]);
      hit = items.filter((/** @type {any} */ i) => i.ask === id || i.request_id === id);
    } else if ((m = /^\/api\/(changes|runs|projects)\/([^/]+)\/(\w+)/.exec(path))) {
      const id = decodeURIComponent(m[2]);
      const key = { changes: "change_id", runs: "run_id", projects: "project_id" }[/** @type {"changes"} */ (m[1])];
      hit = items.filter((/** @type {any} */ i) => i[key] === id);
      if (m[3] === "offer") offers.push(path);
      if (m[3] === "snooze" && /minutes=0\b/.test(query)) {
        for (const i of hit) gone.delete(i.id);
        return [];
      }
    }
    for (const i of hit) gone.add(i.id);
    return hit.map((/** @type {any} */ i) => i.id);
  }
  return {
    gone,
    offers,
    writes,
    /** The items the inbox reads now. */
    items: () => (inbox.items ?? []).filter((/** @type {any} */ i) => !gone.has(i.id)),
    /** @param {any} page */
    async attach(page) {
      await page.route("**/api/**", (/** @type {any} */ r) => {
        const url = new URL(r.request().url());
        if (r.request().method() !== "GET") {
          writes.push(`${r.request().method()} ${url.pathname}`);
          write(url.pathname, url.search);
          return r.fulfill({ json: {} });
        }
        if (url.pathname === "/api/inbox") {
          return r.fulfill({ json: { ...inbox, items: (inbox.items ?? []).filter((/** @type {any} */ i) => !gone.has(i.id)) } });
        }
        return r.fallback();
      });
    },
  };
}

/**
 * The item on screen: the one the address names, else the first in the list
 * (which the inbox shows when the address names none).
 * @param {any} page @param {any[]} items
 */
async function selected(page, items) {
  const h = decodeURIComponent(await page.evaluate(() => location.hash));
  const named = /^#inbox\/item=(.+)$/.exec(h)?.[1];
  if (named) return named;
  const first = (await page.locator("aside.side [role=option] .title").first().textContent().catch(() => ""))?.trim();
  return items.find((i) => i.title === first)?.id ?? null;
}

/**
 * A fresh inbox page with the harness's host behind it, and the item reached
 * by `j` alone — every press counted.
 * @returns {Promise<{ page: any, keys: string[] } | null>}
 */
async function reach(/** @type {any} */ driver, /** @type {ReturnType<typeof host>} */ h, /** @type {any} */ item, /** @type {number} */ max) {
  const page = await driver.page();
  await h.attach(page);
  await page.goto(`${driver.base}#inbox`);
  await page.locator("aside.side [role=option]").first().waitFor({ timeout: 5000 }).catch(() => {});
  await sleep(300);
  await page.evaluate(() => /** @type {HTMLElement | null} */ (document.activeElement)?.blur());
  /** @type {string[]} */
  const keys = [];
  const items = /** @type {any[]} */ (h.items());
  for (let n = 0; n <= max && (await selected(page, items)) !== item.id; n++) {
    await page.keyboard.press("j");
    keys.push("j");
    await sleep(150);
  }
  if ((await selected(page, items)) !== item.id) {
    await page.context().close();
    return null;
  }
  return { page, keys };
}

/**
 * Whether the item left: the bar's host took it out, or the page left the
 * inbox for the item's own place.
 */
async function out(/** @type {any} */ page, /** @type {ReturnType<typeof host>} */ h, /** @type {any} */ item) {
  await sleep(400);
  if (h.gone.has(item.id)) return "answered or put away";
  const hash = decodeURIComponent(await page.evaluate(() => location.hash));
  if (!hash.startsWith("#inbox")) return `opened ${hash.split("/").slice(0, 1).join("")}`;
  return null;
}

/**
 * The keyed ways out an item may have, cheapest first: the keys printed on
 * it, its answer keys, a numbered option, snooze, then moving into the item
 * and tabbing to a control that takes it out.
 * @param {any} item
 */
function ways(item, /** @type {string[]} */ printed) {
  /** @type {{ name: string, keys: string[] | null }[]} */
  const w = printed.map((k) => ({ name: `${k} (printed on the item)`, keys: [k] }));
  const acts = item.actions ?? [];
  if (acts.includes("allow")) w.push({ name: "a (allow)", keys: ["a"] });
  if (acts.includes("deny")) w.push({ name: "d (deny)", keys: ["d"] });
  if ((item.options ?? []).length || (item.form ?? []).length) w.push({ name: "1 (first option)", keys: ["1"] });
  if (acts.includes("snooze")) w.push({ name: "s (snooze)", keys: ["s"] });
  w.push({ name: "Enter, Tab to a control, Enter", keys: null });
  return w;
}

/** @param {any} driver @param {any} budget */
export default async function bar2(driver, budget) {
  /** @type {string[]} */
  const findings = [];
  /** @type {string[]} */
  const measured = [];
  const inbox = recordings().inbox ?? { items: [] };
  const items = inbox.items ?? [];
  if (items.length === 0) return { measured: ["0 items"], findings: ["no-items · the inbox recording holds no item: a bar that answered nothing measured nothing"] };
  if (!items.some(answerable)) findings.push("nothing-answerable · no item in the inbox recording is one Devplane can answer");

  // ── The guard: nothing reachable from an item offers a change ──────────
  {
    const page = await driver.page();
    await page.goto(`${driver.base}#inbox`);
    await page.locator("main#surface").waitFor({ timeout: 5000 });
    const keys = await page.evaluate(async () => {
      const k = await import(/* @vite-ignore */ "/src/lib/keys.ts");
      return k.help("inbox").map((/** @type {any} */ b) => ({ combo: b.combo, label: b.label, surface: b.surface }));
    });
    const surfaces = await registry(page);
    await page.context().close();
    if (!surfaces.some((s) => s.id === "inbox")) findings.push("unreachable · no surface registers as inbox");
    for (const k of keys) if (OFFER.test(k.label)) findings.push(`offers · the key ${k.combo} (${k.label}) is bound in the inbox`);
    const inboxKeys = keys.filter((/** @type {any} */ k) => k.surface === "inbox" && !/^(j|k|Up|Down|G|g g)$/.test(k.combo)).map((/** @type {any} */ k) => k.combo);

    for (const item of items) {
      const h = host(inbox);
      let page = await driver.page();
      await h.attach(page);
      await page.goto(`${driver.base}#inbox/${encodeURIComponent(`item=${item.id}`)}`);
      await page.locator("main#surface .detail").waitFor({ timeout: 5000 }).catch(() => {});
      await sleep(400);
      const controls = await page.evaluate(() =>
        [...document.querySelectorAll("main#surface .detail button, main#surface .detail a, aside.side [aria-selected=true] button, aside.side [aria-selected=true] a")].map((b) =>
          (b.getAttribute("aria-label") || b.textContent || b.getAttribute("title") || "").trim().replace(/\s+/g, " "),
        ),
      );
      for (const c of controls) if (OFFER.test(c)) findings.push(`offers · ${item.title} · its control "${c}"`);
      // The palette over the item.
      await page.keyboard.press("ControlOrMeta+k");
      await page.locator(".palette [role=option]").first().waitFor({ timeout: 3000 }).catch(() => {});
      const entries = await page.locator(".palette [role=option]").allTextContents();
      for (const e of entries) if (OFFER.test(e)) findings.push(`offers · ${item.title} · the palette entry "${e.trim().replace(/\s+/g, " ")}"`);
      await page.keyboard.press("Escape");
      await page.context().close();
      // Every inbox key pressed over the item, on one page, watching for an
      // offer request behind a neutral label.
      {
        page = await driver.page();
        await h.attach(page);
        await page.goto(`${driver.base}#inbox/${encodeURIComponent(`item=${item.id}`)}`);
        await page.locator("main#surface .detail").waitFor({ timeout: 5000 }).catch(() => {});
        await sleep(300);
        for (const k of inboxKeys) {
          const before = h.offers.length;
          await page.evaluate(() => /** @type {HTMLElement | null} */ (document.activeElement)?.blur());
          if (!(await page.evaluate(() => location.hash.startsWith("#inbox")))) {
            await page.goto(`${driver.base}#inbox/${encodeURIComponent(`item=${item.id}`)}`);
            await sleep(300);
          }
          for (const part of k.split(" ")) await page.keyboard.press(part.replace(/^Mod\+/, "ControlOrMeta+"));
          await sleep(200);
          if (h.offers.length > before) findings.push(`offers · ${item.title} · key ${k} sent ${h.offers[h.offers.length - 1]}`);
        }
        await page.context().close();
      }
      // Every control pressed, each on a fresh page.
      const presses = controls.map((c, n) => ({ c, n }));
      for (const p of presses) {
        page = await driver.page();
        await h.attach(page);
        await page.goto(`${driver.base}#inbox/${encodeURIComponent(`item=${item.id}`)}`);
        await page.locator("main#surface .detail").waitFor({ timeout: 5000 }).catch(() => {});
        await sleep(300);
        const before = h.offers.length;
        try {
          {
            const el = page.locator("main#surface .detail button, main#surface .detail a, aside.side [aria-selected=true] button, aside.side [aria-selected=true] a").nth(p.n);
            // A link that leaves the window is not followed; its label was read.
            const href = await el.getAttribute("href", { timeout: 1000 }).catch(() => null);
            if (!href || href.startsWith("#")) await el.click({ timeout: 1500 }).catch(() => {});
          }
          await sleep(400);
        } finally {
          await page.context().close();
        }
        if (h.offers.length > before) findings.push(`offers · ${item.title} · ${p.c} sent ${h.offers[h.offers.length - 1]}`);
      }
    }
    measured.push(`guard: ${items.length} items, their controls, ${inboxKeys.length} inbox keys and the palette over each`);
  }

  // ── The walk: each item out by keys, in the inbox's order ─────────────
  const h = host(inbox);
  for (const item of items) {
    const can = answerable(item);
    /** @type {{ way: string, keys: string[], left: string } | null} */
    let best = null;
    const before = new Set(h.gone);
    const printed = await printedKeys(driver, item);
    for (const w of ways(item, printed)) {
      const reached = await reach(driver, h, item, items.length + 1);
      if (!reached) break;
      const { page, keys } = reached;
      try {
        if (w.keys) {
          for (const k of w.keys) {
            await page.keyboard.press(k);
            keys.push(k);
          }
          const left = await out(page, h, item);
          if (left && (!best || keys.length < best.keys.length)) best = { way: w.name, keys, left };
        } else {
          // Into the item, then Tab through its controls; the first one that
          // takes it out is the way.
          await page.keyboard.press("Enter");
          keys.push("Enter");
          await sleep(150);
          for (let t = 0; t < 12; t++) {
            await page.keyboard.press("Tab");
            keys.push("Tab");
            const here = await page.evaluate(() => {
              const a = document.activeElement;
              if (!a || !a.closest("main#surface .detail")) return null;
              return { tag: a.tagName, label: (a.getAttribute("aria-label") || a.textContent || "").trim().replace(/\s+/g, " ") };
            });
            if (!here || here.tag === "INPUT" || here.tag === "TEXTAREA" || /snooze|undo|copy/i.test(here.label)) continue;
            await page.keyboard.press("Enter");
            const left = await out(page, h, item);
            if (left) {
              keys.push("Enter");
              if (!best || keys.length < best.keys.length) best = { way: `Enter, Tab ×${keys.filter((k) => k === "Tab").length} to "${here.label}", Enter`, keys, left };
              break;
            }
          }
        }
      } finally {
        await page.context().close();
      }
      // Each try starts from the same inbox.
      h.gone.clear();
      for (const g of before) h.gone.add(g);
      if (best && best.keys.length <= (can ? budget.keysPerAnswerable : 1)) break;
    }
    const tag = can ? "answerable" : "not answerable here";
    if (!best) {
      findings.push(`mouse-required · ${item.title} · no keyed way out`);
      measured.push(`${clip(item.title)} · ${tag} · no keyed way out`);
      continue;
    }
    // Answered or put away, it is gone for the next item; opened, it stays.
    if (best.left === "answered or put away") h.gone.add(item.id);
    measured.push(`${clip(item.title)} · ${tag} · ${best.keys.length} key${best.keys.length === 1 ? "" : "s"} (${best.keys.join(" ")}) · ${best.way} → ${best.left}`);
    if (can && best.keys.length > budget.keysPerAnswerable) {
      findings.push(`over-budget · ${item.title} · ${best.keys.length} keys (${best.keys.join(" ")}), budget ${budget.keysPerAnswerable}`);
    }
  }
  return { measured, findings };
}

/** The keys the item prints beside its controls (`data-key`). */
async function printedKeys(/** @type {any} */ driver, /** @type {any} */ item) {
  const page = await driver.page();
  await page.route("**/api/**", (/** @type {any} */ r) => (r.request().method() === "GET" ? r.fallback() : r.fulfill({ json: {} })));
  await page.goto(`${driver.base}#inbox/${encodeURIComponent(`item=${item.id}`)}`);
  await page.locator("main#surface .detail").waitFor({ timeout: 5000 }).catch(() => {});
  await sleep(300);
  const keys = await page.evaluate(() => [...document.querySelectorAll("main#surface .detail [data-key]")].map((e) => e.getAttribute("data-key") ?? "").filter(Boolean));
  await page.context().close();
  return [...new Set(keys)];
}

const clip = (/** @type {string} */ s) => (s.length > 48 ? `${s.slice(0, 47)}…` : s);

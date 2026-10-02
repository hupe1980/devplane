// Bar 4 — no unlabeled state. Every registered surface, in five phases
// (loading, empty, nothing visible, failed, populated), forced by answering the
// host's routes from the page. It fails by surface, phase and finding:
//
//   count-while-loading   a number while nothing has loaded (outside key hints
//                         and the surface's own heading), or a badge beside it
//   error-unrendered      a failed read neither said nor named
//   skeleton-beside-error a skeleton still showing 2 s after the failure
//   verified-unqualified  *verified* on the weakened change without its
//                         qualifier on the same line
//   literal-markdown      a backtick or `**` in prose (code, pre, blockquote
//                         and [data-quoted] are exempt by that marking)
//   absence-equals-zero   *empty* and *nothing visible* read the same
//   unreachable           a surface × phase that could not be opened
//   subject-missing       no surface showed the weakened change at all

import { EXEMPT } from "./budgets.mjs";
import { PHASES, PLANTED, SUBJECT, phase, recorded, registry, sleep, subject } from "./driver.mjs";

/** How long a phase is given to render before it is read. */
const SETTLE = 1000;

/**
 * Where each surface is opened: its own address, and — for a surface opened
 * about a change, an ask or a run — that address on the recorded subject. The
 * change's review is a view of the change, opened as one.
 * @param {import("./driver.mjs").Registered} s
 */
function places(s) {
  /** @type {{ name: string, hash: string, transient?: boolean }[]} */
  const out = [];
  if (s.transient) return [{ name: s.id, hash: "", transient: true }];
  out.push({ name: s.id, hash: `#${s.id}` });
  for (const kind of [s.link, s.holds]) {
    if (!kind) continue;
    const id = subject(kind);
    if (!id) continue;
    const focus = s.link === kind && kind === "ask" ? `ask=${id}` : id;
    if (out.some((p) => p.hash === `#${s.id}/${encodeURIComponent(focus)}`)) continue;
    out.push({ name: `${s.id}/<${kind}>`, hash: `#${s.id}/${encodeURIComponent(focus)}` });
    if (kind === "change") out.push({ name: `${s.id}/<change>/review`, hash: `#${s.id}/${encodeURIComponent(focus)}/review` });
  }
  return out;
}

/**
 * What the page shows, read in the page: the surface's visible text, the
 * numbers outside key hints and the heading, badges, skeletons, prose with
 * markdown left in it, and each *verified* on a line naming the subject.
 * @param {any} page
 * @param {{ heading: string, title: string, says: string, transient?: boolean }} ask
 */
async function read(page, ask) {
  return page.evaluate((/** @type {any} */ a) => {
    // The surface's page and its sidebar list; the palette floats over both.
    const palette = a.transient ? document.querySelector(".palette") : null;
    const main = document.querySelector("main#surface");
    if (!main && !palette) return null;
    const roots = /** @type {HTMLElement[]} */ (palette ? [palette] : [main, document.querySelector("aside.side")].filter(Boolean));
    const visible = (/** @type {Element} */ el) => {
      const r = el.getBoundingClientRect();
      const st = getComputedStyle(el);
      return r.width > 0 && r.height > 0 && st.visibility !== "hidden" && st.display !== "none";
    };
    /** Text nodes that are shown, with their nearest element. */
    const texts = [];
    for (const root of roots) {
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const el = n.parentElement;
        if (!el || !n.textContent?.trim() || !visible(el)) continue;
        if (el.closest("[aria-hidden=true], script, style")) continue;
        texts.push({ t: n.textContent, el });
      }
    }
    const text = roots.map((r) => r.innerText).join(" ").replace(/\s+/g, " ").trim();
    // Numbers: outside key hints, the surface's own heading, and stamped times.
    const numbers = texts
      .filter(({ el }) => !el.closest("kbd, .kbd, time") && el.closest("h1, h2")?.textContent?.trim() !== a.heading)
      .map(({ t }) => t.trim())
      // A count is a number standing alone — not a digit inside an id.
      .filter((t) => /(?<![\w.-])\d+(?![\w-])/.test(t));
    const markdown = texts
      .filter(({ el }) => !el.closest("code, pre, blockquote, [data-quoted]"))
      .map(({ t }) => t.trim())
      .filter((t) => t.includes("`") || t.includes("**"));
    const badges = [...document.querySelectorAll(".badge")].filter(visible).map((b) => b.textContent?.trim() ?? "");
    const busy = roots.flatMap((r) => [...r.querySelectorAll("[aria-busy=true]")]).filter(visible).length;
    // Each *verified* inside a row or header that names the weakened change,
    // and whether the qualifier's words share its line.
    const verified = [];
    let named = false;
    for (const { t, el } of texts) {
      if (t.includes(a.title)) named = true;
      if (!/\bverified\b/i.test(t) || /unverified|not verified/i.test(t)) continue;
      const row = el.closest("[role=row], [role=option], li, tr, header, .head, .row, .facts");
      if (!row || !row.textContent?.includes(a.title)) continue;
      const box = el.getBoundingClientRect();
      // The innermost elements carrying the qualifier's words.
      const quals = [...row.querySelectorAll("*")]
        .filter((q) => q.textContent?.includes(a.says) && visible(q))
        .filter((q) => ![...q.children].some((c) => c.textContent?.includes(a.says)));
      const same = quals.some((q) => {
        const b = q.getBoundingClientRect();
        return b.top < box.bottom && box.top < b.bottom;
      });
      verified.push({ row: row.textContent.replace(/\s+/g, " ").trim().slice(0, 100), same });
    }
    return { text, numbers, markdown, badges, busy, verified, named };
  }, ask);
}

/** @param {any} driver */
export default async function bar4(driver) {
  /** @type {string[]} */
  const findings = [];
  /** @type {string[]} */
  const notes = [];
  const change = recorded("change", SUBJECT);
  const says = change?.qualifier?.says ?? "";
  const first = await driver.page();
  await first.goto(`${driver.base}#inbox`);
  await first.locator("main#surface").waitFor({ timeout: 10_000 }).catch(() => {});
  const surfaces = await registry(first).catch(() => []);
  await first.context().close();
  if (surfaces.length === 0) {
    return { measured: ["0 surfaces visited"], findings: ["measured nothing: the interface registered no surface"], notes };
  }
  if (!change || !says) findings.push(`subject-missing · no recorded change titled "${SUBJECT}" with a weakened check`);

  const exempt = (/** @type {string} */ s, /** @type {string} */ p, /** @type {string} */ rule) =>
    EXEMPT.find((e) => e.surface === s && e.phase === p && (e.rule === "*" || e.rule === rule));
  let visited = 0;
  let unreachable = 0;
  let subjectSeen = false;
  /** @type {Map<string, string>} the *empty* text per place, to compare with *nothing visible* */
  const emptyText = new Map();

  for (const s of surfaces) {
    for (const place of places(s)) {
      for (const ph of PHASES) {
        const where = `${place.name} · ${ph}`;
        if (exempt(s.id, ph, "*")) continue;
        // A host with nothing has no subject to open: a place about one is
        // visited loading, failed and populated.
        if (place.name.includes("<") && (ph === "empty" || ph === "nothing visible")) continue;
        const page = await driver.page();
        const errors = /** @type {string[]} */ ([]);
        page.on("pageerror", (/** @type {Error} */ e) => errors.push(e.message));
        try {
          await phase(page, ph);
          if (place.transient) {
            await page.goto(`${driver.base}#inbox`);
            await page.locator("main#surface").waitFor({ timeout: 5000 });
            await sleep(200);
            await page.keyboard.press("ControlOrMeta+k");
            await page.locator(".palette").first().waitFor({ timeout: 3000 });
          } else {
            await page.goto(`${driver.base}${place.hash}`);
            await page.locator("main#surface").waitFor({ timeout: 5000 });
          }
          await sleep(SETTLE);
          let r = await read(page, { heading: s.heading, title: SUBJECT, says, transient: !!place.transient });
          if (!r) throw new Error("nothing rendered");
          visited += 1;
          if (errors.length) findings.push(`${where} · page-error · ${clip(errors[0])}`);

          if (ph === "loading") {
            if (r.numbers.length && !exempt(s.id, ph, "count-while-loading")) findings.push(`${where} · count-while-loading · ${quote(r.numbers.join(" "))}`);
            if (r.badges.length && !exempt(s.id, ph, "count-while-loading")) findings.push(`${where} · count-while-loading · badge ${quote(r.badges.join(" "))}`);
          }
          if (ph === "failed") {
            const reads = s.reads.some((x) => x.includes("/api/forge"));
            const said = r.text.includes(PLANTED) || /Could not read/i.test(r.text) || /did not answer|is not answering|cannot reach|can't reach/i.test(r.text);
            if (!said && !exempt(s.id, ph, "error-unrendered")) findings.push(`${where} · error-unrendered · planted "${PLANTED} …" not shown`);
            if (reads && !/not signed in to GitHub/.test(r.text) && !exempt(s.id, ph, "error-unrendered")) {
              findings.push(`${where} · error-unrendered · the forge's "not signed in to GitHub" not shown`);
            }
            await sleep(2000 - SETTLE + 200);
            r = (await read(page, { heading: s.heading, title: SUBJECT, says, transient: !!place.transient })) ?? r;
            if (r.busy && !exempt(s.id, ph, "skeleton-beside-error")) findings.push(`${where} · skeleton-beside-error · ${r.busy} skeleton(s) still showing after 2 s`);
          }
          if (ph === "populated") {
            if (r.named) subjectSeen = true;
            for (const v of r.verified) {
              if (!v.same && !exempt(s.id, ph, "verified-unqualified")) findings.push(`${where} · verified-unqualified · ${quote(v.row)} without "${says}" on its line`);
            }
          }
          if (r.markdown.length && !exempt(s.id, ph, "literal-markdown")) findings.push(`${where} · literal-markdown · ${quote(r.markdown[0])}`);
          if (ph === "empty") emptyText.set(place.name, r.text);
          if (ph === "nothing visible" && emptyText.has(place.name) && emptyText.get(place.name) === r.text && !exempt(s.id, ph, "absence-equals-zero")) {
            findings.push(`${where} · absence-equals-zero · reads the same as empty: ${quote(r.text)}`);
          }
        } catch (e) {
          unreachable += 1;
          findings.push(`${where} · unreachable · ${clip(e instanceof Error ? e.message : String(e))}`);
        } finally {
          await page.context().close();
        }
      }
    }
  }
  if (change && says && !subjectSeen) findings.push(`subject-missing · no surface showed "${SUBJECT}" in the populated phase`);
  notes.push("skipped  a place opened about a recorded subject (<change>, <ask>, <run>) · empty and nothing visible — an empty host has no subject to open");
  for (const e of EXEMPT) notes.push(`exempt   ${e.surface} · ${e.phase} · ${e.rule} — ${e.reason}`);
  const placesN = surfaces.reduce((n, s) => n + places(s).length, 0);
  return {
    measured: [
      `${surfaces.length} surfaces (${placesN} places) × ${PHASES.length} phases: ${visited} visited; exempt ${EXEMPT.length}; unreachable ${unreachable}`,
    ],
    findings,
    notes,
  };
}

const clip = (/** @type {string} */ s) => s.split("\n")[0].slice(0, 120);
const quote = (/** @type {string} */ s) => `"${s.replace(/\s+/g, " ").trim().slice(0, 120)}"`;

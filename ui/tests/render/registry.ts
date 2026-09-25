// The registry and the frame around it: every surface registers itself, names
// itself, reads from somewhere real, and the shell names none of them.
import { readdirSync } from "node:fs";
import { surfaces, listed, landing, BANDS, BAND_LABELS, type Feed } from "../../src/lib/surfaces";
import { loadSurfaces } from "../../src/lib/load";
import { all, bind, help } from "../../src/lib/keys";
import ActivityBar from "../../src/shell/ActivityBar.svelte";
import StatusBar from "../../src/shell/StatusBar.svelte";
import Panel from "../../src/shell/Panel.svelte";
import board from "../../fixtures/board.json";
import inbox from "../../fixtures/inbox.json";
import { fail, html, source, walk } from "../harness";

loadSurfaces();
const recorded: Feed = { board, inbox, loaded: true, stale_since: null, error: null };
const empty: Feed = { board: null, inbox: null };
const find = (id: string) => surfaces().find((s) => s.id === id);

// ── Every surface maps any feed to its own props, and names itself ───────
{
  if (surfaces().length < 8) fail("the registry resolved fewer than eight surfaces");
  const ids = new Set<string>();
  for (const s of surfaces()) {
    if (ids.has(s.id)) fail(`two surfaces claim the id ${s.id}`);
    ids.add(s.id);
    if (!s.title.trim()) fail(`${s.id} has no title, so nothing can name it`);
    for (const [name, feed, focus] of [["an empty feed", empty, ""], ["the recorded feed", recorded, "focus"],
      ["an unfamiliar focus", empty, "nonsense/../.."]] as const) {
      try {
        const picked = s.select(feed, focus);
        if (!picked || typeof picked !== "object") fail(`${s.id}.select returned no props for ${name}`);
      } catch (e) {
        fail(`${s.id}.select threw on ${name}: ${String(e)}`);
      }
    }
    try {
      s.status?.(empty);
      s.status?.(recorded);
      s.tab?.(recorded, "focus");
      s.count?.(empty);
    } catch (e) {
      fail(`${s.id}'s status, tab or count threw: ${String(e)}`);
    }
    // A listed surface shows its count only where there is one: unread is not zero.
    if (s.count && s.count(empty) !== null && s.count(empty) !== undefined) fail(`${s.id} counts ${s.count(empty)} before the feed arrived`);
  }
}

// ── The shell opens on what needs you, and the order is chosen ───────────
{
  if (landing()?.id !== "inbox") fail(`the shell opens on ${landing()?.id}, not on what needs you`);
  const order = listed().map((s) => s.id);
  if (order.join() === [...order].sort().join()) fail("nav order is alphabetical, so no one has chosen what comes first");
  for (const s of surfaces()) if (!BANDS.includes(s.band)) fail(`${s.id} is in no band`);
  for (const b of BANDS) if (!listed().some((s) => s.band === b)) fail(`band "${b}" holds no listed surface`);

  // Titles are scanned: a noun, three words at most, first word its own.
  const seen = new Map<string, string>();
  for (const s of surfaces()) {
    const w = s.title.trim().split(/\s+/)[0].toLowerCase().replace(/[^a-z]/g, "");
    if (seen.has(w)) fail(`"${s.title}" and "${seen.get(w)}" both begin with "${w}"`);
    seen.set(w, s.title);
    if (/^(what|why|is|how|which|where|who)\b/i.test(s.title)) fail(`"${s.title}" opens as a question`);
    if (s.title.trim().split(/\s+/).length > 3) fail(`"${s.title}" is more than three words`);
    if (s.nav !== false && !s.icon) fail(`${s.id} is in the activity bar with no icon`);
  }

  // The bands are the CLI's errands, read out of the Rust rather than copied.
  const cli = source("../../src/cli/mod.rs");
  const block = (cli.split("COMMAND_GROUPS")[1] ?? "").split("\n];")[0];
  const groups = new Set([...block.matchAll(/"([A-Z][^"]+)"/g)].map((m) => m[1]));
  if (groups.size < 5) fail(`only ${groups.size} command groups were read from src/cli/mod.rs — this check is inert`);
  for (const band of BANDS) {
    const label = BAND_LABELS[band];
    if (!groups.has(label) && ![...groups].some((g) => g.startsWith(label + ",")))
      fail(`band "${band}" is called "${label}", which is not one of the CLI's command groups`);
  }
  // And the bar names each band as a group, for screen readers.
  const bar = html(ActivityBar, { items: listed(), feed: recorded, current: "inbox", pick: () => {} });
  for (const band of BANDS) if (!bar.includes(`role="group" aria-label="${BAND_LABELS[band]}"`)) fail(`the activity bar does not name the band ${band}`);
  for (const s of listed()) {
    const n = s.count?.(recorded) ?? null;
    const name = n ? `${s.title}, ${n}` : s.title;
    if (!bar.includes(`aria-label="${name}"`)) fail(`the activity bar's ${s.id} button is not named "${name}"`);
  }
  if (!/aria-current="page"/.test(bar)) fail("nothing in the activity bar says which surface is showing");
  if (/class="badge[^"]*">0</.test(html(ActivityBar, { items: listed(), feed: { board: { runs: [], changes: [] }, inbox: { items: [] } }, current: "", pick: () => {} })))
    fail("a surface with nothing in it shows a 0 badge, an alarm that went off");
}

// ── The page heading is declared, and a component of the surface renders it ─
{
  for (const s of surfaces()) {
    if (!s.heading?.trim()) {
      fail(`${s.id} declares no heading`);
      continue;
    }
    const dir = `../src/surfaces/${s.id}/`;
    const found = readdirSync(new URL(dir, import.meta.url)).filter((f) => f.endsWith(".svelte")).some((f) => source(dir + f).includes(s.heading));
    if (!found) fail(`surface "${s.id}" declares the heading "${s.heading}" and no component in its directory renders it`);
  }
  // The editor is a landmark named by the page it holds.
  if (!/<main id="surface"[^>]*aria-label=\{page\?\.heading\}/.test(source("../src/App.svelte"))) fail("the editor landmark is unnamed");
}

// ── Every surface gets its data from somewhere real ──────────────────────
{
  const api = source("../../src/api.rs");
  const served = new Set([...api.matchAll(/\.route\("([^"]+)"/g)].map((m) => m[1]));
  if (served.size < 20) fail(`only ${served.size} routes were read from src/api.rs — this check is inert`);
  for (const s of surfaces()) {
    const takesProps = Object.keys(s.select(recorded, "focus")).length > 0;
    if (!takesProps && !(s.reads ?? []).length) fail(`surface "${s.id}" selects nothing and reads nothing, so it renders its defaults for ever`);
    const dir = `../src/surfaces/${s.id}/`;
    const code = readdirSync(new URL(dir, import.meta.url)).filter((f) => f !== "index.ts").map((f) => source(dir + f)).join("\n");
    for (const route of s.reads ?? []) {
      if (!served.has(route)) fail(`surface "${s.id}" reads ${route}, which src/api.rs does not serve`);
      if (!code.includes(route)) fail(`surface "${s.id}" declares it reads ${route} and none of its components calls it`);
    }
  }
}

// ── Opened about something, a surface lands on it ────────────────────────
{
  const feed: Feed = { board: { changes: [{ id: "w1", title: "most recent", state: "in flight" }, { id: "w2", title: "second", state: "verified" }] }, inbox: null };
  if (find("why")?.select(feed, "run-7").about !== "run-7") fail("the ledger ignores the run it was opened about");
  if (find("change")?.select(feed, "w2").id !== "w2") fail("the change surface ignores which change it was opened about");
  if (find("change")?.tab?.(feed, "w2") !== "second") fail("a change's tab is not called by its title");
  const ib = find("inbox");
  const items = inbox.items;
  const link = ib?.select(recorded, ib.linkFocus?.("ask-9") ?? "") as { wanted?: string; project?: string };
  if (link?.wanted !== "ask-9" || link?.project !== "") fail("an ask link does not land on its row");
  const row = ib?.select(recorded, items[0].id) as { project?: string };
  if (row?.project !== "") fail("opening a row is read as narrowing to a project");
  const narrowed = ib?.select(recorded, "saas") as { project?: string; watching?: unknown };
  if (narrowed?.project !== "saas") fail("the palette's narrow to this project lands nowhere");
  if (!narrowed?.watching) fail("the inbox does not select the board's watching, so its close cannot say what it cannot see");
  for (const id of ["inbox", "board"]) {
    const s = find(id);
    if (s?.select({ ...empty, loaded: false }, "").loaded !== false || s?.select({ ...empty, loaded: true }, "").loaded !== true)
      fail(`${id}.select does not carry the feed's phase, so it cannot tell unread from empty`);
  }
  for (const kind of ["change", "review", "ask", "run"])
    if (surfaces().filter((s) => s.link === kind).length !== 1) fail(`the ${kind} link is not claimed by exactly one surface`);
  if (surfaces().filter((s) => s.holds === "run").length !== 1) fail("no one surface holds a run, so the activity panel opens nothing");
  const marks = surfaces().filter((s) => s.marksLook);
  if (marks.length !== 1 || marks[0].id !== "inbox") fail("a look is marked by something other than the inbox");
  // Unlisted is not unreachable: each has its way in.
  if (!surfaces().some((s) => s.nav === false && s.takesQuery)) fail("search is unlisted and nothing hands it a query");
  for (const s of surfaces().filter((x) => x.nav === false)) {
    const way = s.transient || s.bare || s.link || s.takesQuery;
    if (!way) fail(`${s.id} is unlisted and has no way in`);
  }
}

// ── The frame: status, panel and shell name no surface ───────────────────
{
  const items = surfaces().flatMap((s) => (s.status?.(recorded) ?? []).map((i) => ({ ...i, surface: s.id, title: s.title })));
  const sum = board.summary;
  const bar = html(StatusBar, { items, projects: sum.projects, pulse: "live", bad: false, theme: "auto",
    cycleTheme: () => {}, help: () => {}, go: () => {} });
  for (const want of [`${sum.working} working`, `${sum.needs_you} need you`, `${sum.projects} projects`])
    if (!bar.includes(want)) fail(`the status bar does not say "${want}"`);
  if (/0 failed|0 asked/.test(bar)) fail("an empty bucket is a zero in the status bar rather than absent");
  if (surfaces().some((s) => (s.status?.(empty) ?? []).length > 0)) fail("the status bar counts before the feed arrived");
  const panel = html(Panel, { board, open: null });
  if (!/<button[^>]*disabled/.test(panel)) fail("the activity panel offers rows that open nothing");
  if (!/Watched end to end/.test(html(Panel, { board, open: () => {} })) && !/Sight/.test(panel)) fail("the panel has no sight view");
  const shell = source("../src/App.svelte");
  if (!/q\.get\("surface"\)/.test(shell) || !/s\.link \? q\.get\(s\.link\)/.test(shell)) fail("the shell does not read the window's query generically");
  if (!shell.includes("names nothing this app opens; links are change, review, ask, run, inbox")) fail("an unknown link has no sentence");
  if (!/claimToken\(\);/.test(shell)) fail("the token is not claimed before the address is rewritten");
  if (!/!s\.transient && !s\.bare\) show\(/.test(shell)) fail("an overlay or the answer window becomes a tab");
  if (!/\{#each keys as k/.test(shell) || !/\{k\.label\}/.test(shell)) fail("the shell's help is not rendered from the bindings");
  if (/transition|animation|@keyframes/.test(shell)) fail("the frame animates");
}

// ── Keys: every binding is labelled, answered, and bound once ────────────
{
  let threw = "";
  try {
    bind({ surface: "inbox", combo: "j", action: "again", label: "a second next" });
  } catch (e) {
    threw = e instanceof Error ? e.message : String(e);
  }
  if (!threw.includes("bound twice")) fail(`a duplicate binding was not refused: ${threw}`);
  threw = "";
  try {
    bind({ surface: "inbox", combo: "?", action: "mine", label: "help, again" });
  } catch (e) {
    threw = e instanceof Error ? e.message : String(e);
  }
  if (!threw.includes("rebinds")) fail(`a surface rebinding a global was not refused: ${threw}`);
  for (const combo of ["Down", "j", "Up", "k", "Enter"])
    if (!help("inbox").some((b) => b.combo === combo && b.surface === "inbox" && b.label.trim())) fail(`the inbox does not bind ${combo} with a label`);
  if (!all().some((b) => b.surface === "global" && b.combo === "Mod+k" && b.action === "open-palette")) fail("nothing opens the palette");
  // Every key's action has a listener.
  const code = walk("../src/").filter((f) => /\.(svelte|ts)$/.test(f) && !f.includes("/wire/")).map((f) => source(f)).join("\n");
  const answered = new Set([...code.matchAll(/onAction\("([a-z-]+)"/g)].map((m) => m[1]));
  for (const b of all()) if (!answered.has(b.action)) fail(`${b.combo} on ${b.surface} runs "${b.action}", which nothing answers`);
}

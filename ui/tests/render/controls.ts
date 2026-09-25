// The workbench's own controls: each decides something a person relies on, so
// each decision is exported and asserted here — a server render cannot click.
import { createRawSnippet } from "svelte";
import Grid, { nextSort, sorted, type Column } from "../../src/lib/ui/Grid.svelte";
import Tabs, { step } from "../../src/lib/ui/Tabs.svelte";
import Pill, { tone } from "../../src/lib/ui/Pill.svelte";
import Timeline from "../../src/lib/ui/Timeline.svelte";
import Palette, { rank, score, type Entry } from "../../src/surfaces/palette/Palette.svelte";
import New, { startable, type Target } from "../../src/surfaces/new/New.svelte";
import { fail, html, source, visible, walk } from "../harness";

// ── The grid never re-sorts what it was handed until a header is clicked ──
{
  type Row = { id: string; n: number | null };
  const rows: Row[] = [{ id: "c", n: 2 }, { id: "a", n: null }, { id: "b", n: 1 }];
  const columns: Column<Row>[] = [{ key: "id", label: "Id", sort: (r) => r.id }, { key: "n", label: "N", sort: (r) => r.n }, { key: "x", label: "X" }];
  if (sorted(rows, columns, null) !== rows) fail("the grid re-orders its rows with no sort asked for");
  const ids = (r: Row[]) => r.map((x) => x.id).join("");
  // A click: ascending, again: descending, a third time: the host's order.
  let by = nextSort(null, columns[0]);
  if (ids(sorted(rows, columns, by)) !== "abc") fail("the first click on a header does not sort ascending");
  by = nextSort(by, columns[0]);
  if (ids(sorted(rows, columns, by)) !== "cba") fail("the second click does not reverse");
  by = nextSort(by, columns[0]);
  if (by !== null || sorted(rows, columns, by) !== rows) fail("the third click does not return to the host's order");
  // A missing value sorts last in both directions, never as a zero.
  if (ids(sorted(rows, columns, { key: "n", dir: 1 })) !== "bca" || ids(sorted(rows, columns, { key: "n", dir: -1 })) !== "cba")
    fail("a missing value is sorted as a number");
  if (nextSort(null, columns[2]) !== null) fail("a column that declares no sort sorts anyway");

  // Rendered: in the order given, grouped with a count on every group, headers
  // announcing no sort.
  const cell = createRawSnippet((r: () => Row, c: () => Column<Row>) => ({ render: () => `<span>${c().key}:${r().id}</span>` }));
  const out = html(Grid, { id: "t", columns, rows, key: (r: Row) => r.id, cell, label: "things" });
  const order = [...out.matchAll(/data-key="([a-z])"/g)].map((m) => m[1]).join("");
  if (order !== "cab") fail(`the grid rendered its rows as ${order}, not in the order it was handed`);
  if ((out.match(/aria-sort="none"/g) ?? []).length !== columns.length) fail("a header claims a sort nobody asked for");
  if (!/role="grid"[^>]*aria-label="things"/.test(out)) fail("the grid is not a labelled grid");
  const grouped = html(Grid, { id: "g", columns, rows, key: (r: Row) => r.id, cell, group: (r: Row) => (r.n ? "some" : "none") });
  if (!/some[\s\S]*?class="n[^"]*">2</.test(grouped) || !/none[\s\S]*?class="n[^"]*">1</.test(grouped))
    fail("a group of rows is shown without its count, so folding it would hide rows without a number");
}

// ── Tabs: arrows move and wrap, Home and End go to the ends ──────────────
{
  const ids = ["overview", "tasks", "review"];
  const cases: Array<[string, string, string | null]> = [
    ["overview", "ArrowRight", "tasks"], ["review", "ArrowRight", "overview"], ["overview", "ArrowLeft", "review"],
    ["tasks", "Home", "overview"], ["tasks", "End", "review"], ["tasks", "x", null],
  ];
  for (const [from, key, to] of cases)
    if (step(ids, from, key) !== to) fail(`the tab strip moves ${key} from ${from} to ${step(ids, from, key)}, not ${to}`);
  const out = html(Tabs, { tabs: [{ id: "a", label: "Alpha", count: 3 }, { id: "b", label: "Beta", count: null }], active: "a", label: "views" });
  if (!/role="tablist" aria-label="views"/.test(out)) fail("the tab strip is not a labelled tablist");
  if ((out.match(/tabindex="0"/g) ?? []).length !== 1) fail("the tab strip is not one tab stop");
  if (!/aria-selected="true"[^>]*>[\s\S]*?Alpha/.test(out)) fail("the active tab is not selected");
  // A count that is absent is absent, not a zero.
  if (/Beta[\s\S]{0,60}class="count/.test(out)) fail("a tab with no count shows one");
  if (!/Alpha[\s\S]{0,80}>3</.test(out)) fail("a tab's count is not shown");
}

// ── The only green is verified ───────────────────────────────────────────
{
  for (const w of ["verified", "passed"]) if (tone(w) !== "done") fail(`"${w}" is not the verified green`);
  for (const w of ["completed", "done", "allow", "merged", "archived", "clean", "offered", "trusted", "declared", "in flight", "live"])
    if (tone(w) === "done") fail(`"${w}" is painted the verified green, and it is not a gate that passed`);
  if (tone("failed") !== "fail" || tone("needs you") !== "wait" || tone("working") !== "work") fail("the state families are mis-toned");
  if (tone("hibernating") !== "none" || tone(null) !== "none") fail("a word the table does not know is guessed into a family");
  // The word is always printed; the colour only says which family.
  const out = html(Pill, { word: "completed" });
  if (!out.includes("completed") || /class="pill done/.test(out)) fail("a pill for an agent's own word is green or wordless");
  // Green is spent only in the files that draw a verified or passed fact.
  const allowed = new Set(["lib/State.svelte", "lib/ui/Pill.svelte", "lib/ui/Props.svelte", "lib/ui/Tabs.svelte",
    "lib/ui/Timeline.svelte", "surfaces/change/Stepper.svelte", "surfaces/change/Doc.svelte", "surfaces/change/Overview.svelte",
    "surfaces/change/Tasks.svelte", "surfaces/change/Gates.svelte", "surfaces/why/Why.svelte"]);
  for (const f of walk("../src/").filter((x) => /\.(svelte|ts|css)$/.test(x) && !x.includes("/wire/"))) {
    const rel = f.replace("../src/", "");
    if (/var\(--done\)/.test(source(f)) && !allowed.has(rel)) fail(`${rel} spends --done, which is reserved for verified`);
  }
  if ((source("../src/lib/State.svelte").match(/var\(--done\)/g) ?? []).length !== 1)
    fail("the state table does not pair --done with verified exactly once");
}

// ── The palette: fuzzy, in a stable order, and `>` narrows to actions ────
{
  const e = (kind: Entry["kind"], label: string): Entry => ({ kind, label, icon: "dot", go: () => {} });
  const entries = [e("project", "saas"), e("action", "Start a new change"), e("change", "Rate-limit the login route"),
    e("surface", "Go to Sessions"), e("action", "Go to the changes"), e("change", "Retry only on 5xx")];
  // Prefix beats word start beats inside beats scattered letters.
  const s = [score("ret", "retry only"), score("onl", "retry only"), score("try", "retry only"), score("rty", "retry only")];
  if (!(s[0]! < s[1]! && s[1]! < s[2]! && s[2]! < s[3]!)) fail(`the palette's match order is not prefix, word, inside, scattered: ${s}`);
  if (score("zz", "retry") !== null) fail("the palette matches letters that are not there");
  const r = rank(entries, "re").map((x) => x.label);
  if (r[0] !== "Retry only on 5xx" || r[1] !== "Rate-limit the login route") fail(`changes are not first, best match first: ${r}`);
  const kinds = rank(entries, "").map((x) => x.kind);
  if (kinds.join() !== "change,change,surface,action,action,project") fail(`the palette's groups are out of order: ${kinds}`);
  const acts = rank(entries, "> go");
  if (acts.length !== 1 || acts[0].label !== "Go to the changes") fail("`>` does not narrow to actions");
  if (rank(entries, ">").some((x) => x.kind !== "action")) fail("`>` alone lists more than actions");
  const out = html(Palette, { from: "#inbox", changes: [{ id: "c9", title: "the route" }], projects: [{ id: "p", name: "saas" }] });
  for (const want of ["the route", "saas", "Go to"]) if (!out.includes(want)) fail(`the palette cannot reach ${want}`);
  if (!/role="listbox"/.test(out) || !/aria-label="find by name"/.test(out)) fail("the palette's field or list is unlabelled");
}

// ── A new change: Start is enabled only when nothing is refused ──────────
{
  const ok = (name: string): Target => ({ asked: name, name, root: `/r/${name}`, refusal: null, says: "a worktree on its own branch", notes: [] });
  const no = (name: string): Target => ({ ...ok(name), refusal: "untrusted", says: "not trusted — devplane trust /r/x" });
  if (!startable([ok("a"), ok("b")], false, "")) fail("two projects that can both start are not startable");
  if (startable([ok("a"), no("b"), ok("c")], false, "")) fail("Start is enabled with one project refused");
  if (startable(null, false, "") || startable([], false, "")) fail("Start is enabled before the host answered");
  if (startable([ok("a")], true, "") || startable([ok("a")], false, "timed out")) fail("Start is enabled while checking or after a failed check");
  const refused = html(New, { targets: [ok("a"), no("b")] });
  if (!/<button type="submit"[^>]*disabled/.test(refused)) fail("the Start button is enabled while a project is refused");
  if (!refused.includes("not trusted — devplane trust /r/x")) fail("a refusal is not said by project");
  if (!/1 refused — nothing will be created/.test(refused)) fail("the form does not say that nothing will be created");
  if (/<button type="submit"[^>]*disabled/.test(html(New, { targets: [ok("a")] }))) fail("the Start button is disabled with nothing refused");
  if (!source("../src/surfaces/new/New.svelte").includes('"/api/changes/preflight"')) fail("the preflight is not the host's");
}

// ── The timeline is a picture of when, never of how good ─────────────────
{
  const marks = [
    { at: "2026-09-25T10:00:00Z", lane: "change", label: "started", tone: "work" },
    { at: "2026-09-25T10:20:00Z", lane: "gates", label: "check attempt 1: failed", tone: "fail" },
    { at: "2026-09-25T10:40:00Z", lane: "gates", label: "check attempt 2: passed", tone: "done" },
  ];
  const out = html(Timeline, { marks, lanes: ["change", "gates"] });
  if (/<svg|<polyline|<path|<line/.test(out)) fail("the timeline draws a line through its marks, which reads as a trend");
  if ((out.match(/<button[^>]*class="mark/g) ?? []).length !== 3) fail("each event is not its own mark");
  for (const m of marks) if (!out.includes(`aria-label="${m.label} at`)) fail(`the mark "${m.label}" has no words`);
  const text = visible(out);
  if (/\d\s*%|total|average|score|trend/i.test(text)) fail("the timeline states an aggregate");
  if (!html(Timeline, { marks: [], lanes: [] }).includes("Nothing has happened to it yet.")) fail("an empty timeline is a blank");
}

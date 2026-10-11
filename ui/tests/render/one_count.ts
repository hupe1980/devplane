// One number, one name: the activity bar's inbox badge and the status bar
// show the same count under the same words, on every recorded fixture set and
// in every state the inbox passes through (answered, snoozed, held,
// narrowed). Both read `inboxCount`, which reads the host's `count` — the
// figure the tray reads from the same composition. A snoozed item is counted
// where it is folded and listed on request.
import { surfaces, type Feed } from "../../src/lib/surfaces";
import { loadSurfaces } from "../../src/lib/load";
import { COUNT_WORD, inboxCount } from "../../src/surfaces/inbox/index";
import ActivityBar from "../../src/shell/ActivityBar.svelte";
import StatusBar from "../../src/shell/StatusBar.svelte";
import List from "../../src/surfaces/inbox/List.svelte";
import Inbox from "../../src/surfaces/inbox/Inbox.svelte";
import Snoozed from "../../src/surfaces/inbox/Snoozed.svelte";
import { fail, html, source, visible } from "../harness";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Json = any;

loadSurfaces();
const inbox = surfaces().find((s) => s.id === "inbox")!;

/// Every fixture set: the populated recording and each forced phase.
const files = import.meta.glob(["../../fixtures/{board,inbox}.json", "../../fixtures/phases/*/{board,inbox}.json"], {
  eager: true,
  import: "default",
}) as Record<string, Json>;
const sets = new Map<string, { board: Json; inbox: Json }>();
for (const [path, body] of Object.entries(files)) {
  const parts = path.split("/");
  const name = parts.includes("phases") ? parts[parts.length - 2] : "populated";
  const file = parts[parts.length - 1].replace(".json", "") as "board" | "inbox";
  sets.set(name, { ...(sets.get(name) ?? { board: null, inbox: null }), [file]: body });
}
if (!["populated", "empty", "nothing-visible"].every((n) => sets.has(n))) fail(`a fixture set is missing: ${[...sets.keys()]}`);

/// The states one recording passes through, each as the host would send it.
function states(rec: { board: Json; inbox: Json }): Array<[string, Feed, string]> {
  const base = rec.inbox;
  const items: Json[] = base?.items ?? [];
  const feed = (i: Json): Feed => ({ board: rec.board, inbox: i, loaded: true, stale_since: null, error: null });
  const out: Array<[string, Feed, string]> = [["as recorded", feed(base), ""]];
  if (items.length) {
    const rest = items.slice(1);
    out.push(["after an answer", feed({ ...base, items: rest, count: rest.length }), ""]);
    const row = { id: items[0].id, kind: items[0].kind, title: items[0].title, returns: "activity", says: "returns when the session next reports activity", back: "/api/runs/r/snooze?minutes=0&kind=x" };
    out.push(["after a snooze", feed({ ...base, items: rest, count: rest.length, snoozed: (base.snoozed ?? 0) + 1, snoozed_rows: [...(base.snoozed_rows ?? []), row] }), ""]);
    const held = { ask: items[0].ask ?? "a", run: "r", title: items[0].title, says: "drop it", until: new Date(Date.now() + 4000).toISOString(), undo: "/api/asks/a/undo" };
    out.push(["with an answer held", feed({ ...base, items: rest, count: rest.length, held }), ""]);
    // Narrowed: the address names a project; the feed is still the whole inbox.
    out.push(["narrowed", feed(base), "some-project"]);
  }
  return out;
}

for (const [set, rec] of sets) {
  for (const [state, feed, focus] of states(rec)) {
    const where = `${set}, ${state}`;
    const n = inbox.count?.(feed) ?? null;
    if (n !== inboxCount(feed)) fail(`${where}: the badge does not read the one count`);
    const status = inbox.status?.(feed) ?? [];
    const mine = status[0];
    if (n === null) {
      if (status.length) fail(`${where}: the status bar counts an inbox that was never read`);
      continue;
    }
    if (!mine || mine.n !== n) fail(`${where}: the badge says ${n}, the status bar ${mine?.n}`);
    if (mine?.word !== COUNT_WORD || inbox.countWord !== COUNT_WORD) fail(`${where}: the badge and the status bar use different words`);
    // Any other inbox number in the status bar names what it counts.
    for (const other of status.slice(1)) if (other.word === COUNT_WORD) fail(`${where}: a second count is called ${COUNT_WORD}`);
    // The number as each bar renders it.
    const bar = html(ActivityBar, { items: [inbox], feed, current: "inbox", pick: () => {} });
    if (n > 0 && !bar.includes(`aria-label="Inbox, ${n} ${COUNT_WORD}"`)) fail(`${where}: the activity bar does not say ${n} ${COUNT_WORD}`);
    if (n > 0 && !new RegExp(`class="badge[^"]*"[^>]*>${n > 99 ? "99\\+" : n}<`).test(bar)) fail(`${where}: the activity bar's badge does not print ${n}`);
    if (n === 0 && /class="badge/.test(bar)) fail(`${where}: the activity bar prints a badge for zero`);
    const items = status.map((i) => ({ ...i, surface: "inbox", title: "Inbox" }));
    const foot = visible(html(StatusBar, { items, pulse: "live", bad: false, theme: "auto", cycleTheme: () => {}, help: () => {}, go: () => {} }));
    if (!new RegExp(`\\b${n} ${COUNT_WORD}\\b`).test(foot)) fail(`${where}: the status bar does not say ${n} ${COUNT_WORD}`);
    // The narrowed address still counts the whole inbox.
    if (focus && inbox.count?.(feed) !== (feed.inbox as Json).items.length) fail(`${where}: a narrowed address changed the badge`);
    // What a snooze hid is counted beside the list, and listed on request.
    const props = inbox.select(feed, focus) as Json;
    const side = visible(html(List, { ...props, open: () => {} }));
    const snoozed = (feed.inbox as Json).snoozed ?? 0;
    if (snoozed > 0 && !side.includes(`${snoozed} snoozed`)) fail(`${where}: ${snoozed} snoozed, and the list does not count them`);
  }
}

// ── The host's number is the one the tray and the window read ────────────
{
  const index = source("../src/surfaces/inbox/index.ts");
  if (!/count: inboxCount/.test(index) || !/n = inboxCount\(feed\)/.test(index)) fail("the badge and the status bar do not read one function");
  const tray = source("../../src/app/tray.rs");
  if (!/view::inbox_count\(/.test(tray)) fail("the tray does not read the inbox's one count");
  if (/has_answer_path/.test(tray)) fail("the tray counts by a rule of its own");
  const view = source("../../src/view.rs");
  if (!/attention::count\(/.test(view)) fail("the inbox payload's count is not the shared one");
}

// ── The snoozed list: each row says when it returns and brings it back ───
{
  const rows = [
    { id: "r:context_high", kind: "context_high", title: "Context 89% full", returns: "activity", says: "returns when the session next reports activity", back: "/api/runs/r/snooze?minutes=0&kind=context_high" },
    { id: "c:change_broken", kind: "change_broken", title: "The worktree is gone", returns: "activity", says: "the session has ended, so it has not returned", back: "/api/changes/c/snooze?minutes=0&kind=change_broken" },
  ];
  const closed = visible(html(Snoozed, { count: 2, rows }));
  if (!/2 snoozed/.test(closed) || !/list them/.test(closed)) fail("the snoozed count has no way to list them");
  if (html(Snoozed, { count: 0, rows: [] }).includes("snoozed")) fail("zero snoozed is said");
  const src = source("../src/surfaces/inbox/Snoozed.svelte");
  if (!/bring back/.test(src) || !/bringBack\(/.test(src)) fail("a snoozed row has no way back");
  if (!/r\.says/.test(src)) fail("a snoozed row does not say when it returns");
  // Everything snoozed: *Clear.* still says what is hidden.
  const clear = visible(html(Inbox, { items: [], loaded: true, snoozed: 2, snoozed_rows: rows }));
  if (!clear.includes("2 snoozed")) fail("an inbox cleared by snoozing reads as calm");
  const acts = source("../src/surfaces/inbox/actions.ts");
  if (!/\?until=activity&kind=/.test(acts)) fail("s does not snooze until the next activity");
  if (!/\?minutes=0&kind=/.test(acts)) fail("taking a snooze back brings back more than the one kind");
}

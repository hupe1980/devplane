import { isArchived } from "../../lib/State.svelte";
import { register, phase } from "../../lib/surfaces";
import { bind, bindList, onAction } from "../../lib/keys";
import { go } from "../../lib/route";

type Brief = { id: string; title: string; state: string };

// The changes: the product's home noun. The sidebar lists them by project;
// each opens in the editor as a document with six views.
bind({ surface: "global", combo: "g c", action: "go-changes", label: "go to the changes" });
bind({ surface: "change", combo: "r", action: "review", label: "review the change" });
// The list keys move the sidebar's changes (answered in `List.svelte`); in
// the Review tab `j` and `k` are the review's (`change:review`).
bindList("change", "open the change as a tab of its own");
// The six views, by the physical key: ⌥1 … ⌥6 on a Mac.
const VIEWS = ["overview", "tasks", "review", "gates", "ledger", "agent"];
VIEWS.forEach((v, i) =>
  bind({ surface: "change", combo: `Alt+${i + 1}`, action: `view-${v}`, label: `show the ${v}` }),
);
onAction("go-changes", () => {
  go("#change");
  return true;
});

// One key each. `s` marks a hunk in this browser, keyed by the hunk's
// content so a rewritten hunk is unmarked again; a weakened line inside that
// hunk is also recorded as read, durably, by the host; `request a fix` is a message
// to the agent, not a rejection. There is no *accept* mark: one durable seen
// record is the host's to keep, not a second verdict in the browser.
const KEYS: Array<[string, string, string]> = [
  ["j", "hunk-next", "next hunk"],
  ["k", "hunk-prev", "previous hunk"],
  ["n", "file-next", "next file"],
  ["p", "file-prev", "previous file"],
  ["s", "hunk-seen", "mark this hunk (in this browser, until it changes); a weakened line in it is recorded as read by the host"],
  ["f", "hunk-fix", "request a fix: a message to the change's latest run, quoting this hunk"],
  ["x", "hunk-expand", "expand this file's collapsed formatter-only hunks"],
  ["v", "diff-mode", "switch between unified and side-by-side"],
  ["1", "tab-risk", "read by risk"],
  ["2", "tab-intent", "read by intent"],
];
// The review pane's keys, in a change's Review tab (the change's `review` mode).
for (const [combo, action, label] of KEYS) bind({ surface: "change:review", combo, action, label });

const briefs = (feed: { board: unknown | null }): Brief[] =>
  (feed.board as { changes?: Brief[] } | null)?.changes ?? [];

register({
  id: "change",
  icon: "change",
  title: "Changes",
  heading: "Changes",
  band: "attention",
  order: 1,
  // `devplane://change/<id>` lands here on that change.
  link: "change",
  // As many as the sidebar lists: archived changes are kept, not counted.
  count: (feed) => {
    const b = feed.board as { changes?: Brief[] } | null;
    return b?.changes ? b.changes.filter((c) => !isArchived(c.state)).length : null;
  },
  tab: (feed, focus) => briefs(feed).find((c) => c.id === focus)?.title ?? "Change",
  select: (feed, focus) => {
    const all = briefs(feed);
    return {
      id: focus,
      all,
      brief: all.find((c) => c.id === focus) ?? null,
      loaded: phase(feed).loaded,
    };
  },
  side: () => import("./List.svelte"),
  document: true,
  page: () => import("./Doc.svelte"),
});

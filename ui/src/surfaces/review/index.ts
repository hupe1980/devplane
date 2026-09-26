// The review surface registers itself and binds its own keys. It is reached
// from its change and from a `?review=<id>` link.
import { register } from "../../lib/surfaces";
import { bind } from "../../lib/keys";
import Review from "./Review.svelte";

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
// The same keys on the review page and a change's Review tab: one pane.
for (const scope of ["review", "change"]) {
  for (const [combo, action, label] of KEYS) bind({ surface: scope, combo, action, label });
}

register({
  id: "review",
  title: "Review",
  heading: "Review",
  band: "happening",
  order: 1,
  // Not in the nav: a review is of one change, reached from it.
  nav: false,
  link: "review",
  // `#review/<id>` names the change, and the surface loads that one.
  tab: (feed, focus) => {
    const c = ((feed.board as { changes?: Array<{ id: string; title?: string }> } | null)?.changes ?? []).find((x) => x.id === focus);
    return c?.title ? `Review: ${c.title}` : "Review";
  },
  select: (feed, focus) => {
    const b = feed.board as { changes?: unknown[] } | null;
    const c = (b?.changes as Array<{ id: string; title?: string }> | undefined)?.find((x) => x.id === focus);
    return { chosen: focus, title: c?.title ?? "" };
  },
  component: Review,
});

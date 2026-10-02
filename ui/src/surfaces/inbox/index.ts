import { unseen } from "../../lib/sight";
import { register, phase } from "../../lib/surfaces";
import { bind, bindList, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import { address } from "./place.svelte";
import { bindItemKeys } from "./keys";
import type { Feed } from "../../lib/surfaces";

/// The inbox's one number: what the activity bar's badge, the status bar and
/// the tray all show, under the words `COUNT_WORD`. The host composes it
/// (`count` on `/api/inbox`: the whole inbox's listed rows, without the item
/// a held answer belongs to) and the tray reads the same composition; a
/// recording older than that field is the list it was read with.
export function inboxCount(feed: Feed): number | null {
  const i = feed.inbox as { items?: unknown[]; count?: number } | null;
  if (!i?.items) return null;
  return typeof i.count === "number" ? i.count : i.items.length;
}
/// What the number counts, said the same way in the badge's label, the
/// status bar and the tray's tooltip.
export const COUNT_WORD = "need you";

// The list keys (answered by `Inbox.svelte` through `lib/cursor`), and the
// way here from anywhere.
bindList("inbox", "move into the item (Tab then reaches its controls)");
// The keys on the item on screen — one list, shared with the answer window
// (`keys.ts`). There is no dismiss or mark-read: the host has neither for an
// inbox item. No key here offers a change.
bindItemKeys("inbox");
bind({ surface: "global", combo: "g i", action: "go-inbox", label: "go to the inbox" });
onAction("go-inbox", () => {
  go("#inbox");
  return true;
});

register({
  id: "inbox",
  icon: "inbox",
  title: "Inbox",
  heading: "What needs you",
  band: "attention",
  order: 0,
  // Opening this is a look; opening anything else is not.
  marksLook: true,
  // What needs you, right of the status bar: the inbox's one number, the
  // badge's. Asks that outlived their sessions are the host's own number
  // under their own noun, absent when there are none.
  status: (feed) => {
    const n = inboxCount(feed);
    if (n === null) return [];
    const s = (feed.board as { summary?: { asks_waiting?: number } } | null)?.summary;
    return [
      { n, word: COUNT_WORD, icon: "inbox", tone: "wait" as const, end: true },
      ...(s?.asks_waiting ? [{ n: s.asks_waiting, word: "asks that outlived their sessions", icon: "question", tone: "wait" as const, end: true }] : []),
    ];
  },
  count: inboxCount,
  countWord: COUNT_WORD,
  // A link naming an ask — `devplane://ask/<id>` — lands here on that row.
  link: "ask",
  linkFocus: (id) => `ask=${id}`,
  select: (feed, focus) => {
    const b = feed.inbox as
      | {
          items?: Array<{ id: string }>;
          folded?: unknown[];
          inhibited?: unknown[];
          close?: unknown;
          held?: unknown;
          landed?: unknown;
          snoozed?: number;
          snoozed_rows?: unknown[];
        }
      | null;
    const items = b?.items ?? [];
    // `#inbox/item=<id>` is a row; `#inbox/ask=<id>` a link naming one;
    // anything else is a project the host narrows to.
    const { item, wanted, project } = address(focus ?? "", items);
    // The board's sight, so *Clear.* names what it cannot see.
    const watching = (feed.board as { watching?: unknown } | null)?.watching ?? null;
    return {
      items,
      folded: b?.folded ?? [],
      inhibited: b?.inhibited ?? [],
      close: b?.close ?? null,
      held: b?.held ?? null,
      landed: b?.landed ?? null,
      snoozed: b?.snoozed ?? 0,
      snoozed_rows: b?.snoozed_rows ?? [],
      project,
      item,
      wanted,
      watching,
      blind: unseen(feed.board),
      ...phase(feed),
    };
  },
  tab: (feed, focus) => {
    const items = (feed.inbox as { items?: Array<{ id: string; title: string }> } | null)?.items ?? [];
    const { item, project } = address(focus, items);
    return items.find((i) => i.id === item)?.title ?? (project ? `Inbox: ${project}` : "Inbox");
  },
  side: () => import("./List.svelte"),
  page: () => import("./Inbox.svelte"),
});

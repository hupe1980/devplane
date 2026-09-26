import { register, phase } from "../../lib/surfaces";
import { bind, bindList, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import Inbox from "./Inbox.svelte";
import List from "./List.svelte";
import { address } from "./place.svelte";

// The list keys (answered by `Inbox.svelte` through `lib/cursor`), and the
// way here from anywhere.
bindList("inbox", "move into the item (Tab then reaches its controls)");
bind({ surface: "inbox", combo: "a", action: "inbox-allow", label: "allow the permission on screen" });
bind({ surface: "inbox", combo: "d", action: "inbox-deny", label: "deny the permission on screen" });
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
  // What needs you, right of the status bar: the inbox rows a person can
  // answer from here — the rule the answer window's list uses (the host's
  // `needs_you` narrowing), so the two never disagree. Asks that outlived
  // their sessions are the host's own number, absent when there are none.
  status: (feed) => {
    const i = feed.inbox as { items?: Array<{ ask?: string | null; options?: unknown[]; actions?: string[] }> } | null;
    if (!i?.items) return [];
    const s = (feed.board as { summary?: { asks_waiting?: number } } | null)?.summary;
    const answerable = i.items.filter(
      (x) => !!x.ask || (x.options ?? []).length > 0 || (x.actions ?? []).some((a) => a !== "open" && a !== "snooze"),
    ).length;
    return [
      { n: answerable, word: "need you", icon: "inbox", tone: "wait" as const, end: true },
      ...(s?.asks_waiting ? [{ n: s.asks_waiting, word: "asked, session gone", icon: "question", tone: "wait" as const, end: true }] : []),
    ];
  },
  // The badge is everything in the inbox — failed gates and reports too —
  // and is labelled as such beside the icon.
  count: (feed) => {
    const i = feed.inbox as { items?: unknown[] } | null;
    return i?.items?.length ?? null;
  },
  // A link naming an ask — `devplane://ask/<id>` — lands here on that row.
  link: "ask",
  linkFocus: (id) => `ask=${id}`,
  select: (feed, focus) => {
    const b = feed.inbox as
      | { items?: Array<{ id: string }>; folded?: unknown[]; inhibited?: unknown[]; close?: unknown }
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
      project,
      item,
      wanted,
      watching,
      ...phase(feed),
    };
  },
  tab: (feed, focus) => {
    const items = (feed.inbox as { items?: Array<{ id: string; title: string }> } | null)?.items ?? [];
    const { item, project } = address(focus, items);
    return items.find((i) => i.id === item)?.title ?? (project ? `Inbox: ${project}` : "Inbox");
  },
  side: List,
  component: Inbox,
});

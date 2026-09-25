import { register, phase } from "../../lib/surfaces";
import { bind, bindList, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import Inbox from "./Inbox.svelte";
import List from "./List.svelte";

// The list keys, and the way here from anywhere.
bindList("inbox");
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
  // What needs you, right of the status bar; asks that outlived their
  // sessions are their own number, and absent when there are none.
  status: (feed) => {
    const s = (feed.board as { summary?: { needs_you?: number; asks_waiting?: number } } | null)?.summary;
    if (!s) return [];
    return [
      { n: s.needs_you ?? 0, word: "need you", icon: "inbox", tone: "wait" as const, end: true },
      ...(s.asks_waiting ? [{ n: s.asks_waiting, word: "asked", icon: "question", tone: "wait" as const, end: true }] : []),
    ];
  },
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
    // `#inbox/<id>` is a row; `#inbox/ask=<id>` a link naming one; anything
    // else is a project the host narrows to.
    const wanted = focus?.startsWith("ask=") ? focus.slice("ask=".length) : "";
    const project = focus && !wanted && !items.some((i) => i.id === focus) ? focus : "";
    // The board's sight, so *Clear.* names what it cannot see.
    const watching = (feed.board as { watching?: unknown } | null)?.watching ?? null;
    return {
      items,
      folded: b?.folded ?? [],
      inhibited: b?.inhibited ?? [],
      close: b?.close ?? null,
      project,
      wanted,
      watching,
      ...phase(feed),
    };
  },
  tab: (feed, focus) => {
    const items = (feed.inbox as { items?: Array<{ id: string; title: string }> } | null)?.items ?? [];
    return items.find((i) => i.id === focus)?.title ?? "Inbox";
  },
  side: List,
  component: Inbox,
});

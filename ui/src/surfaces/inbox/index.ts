import { register } from "../../lib/surfaces";
import Inbox from "./Inbox.svelte";

register({
  id: "inbox",
  title: "Inbox",
  heading: "What needs you",
  band: "attention",
  order: 0,
  // **Every action the daemon can offer has a home here, or is named as a
  // command because a browser cannot do it.** This listed five while the daemon
  // offered thirteen, and the port inventory read as complete throughout — a
  // declaration is not an implementation.
  count: (feed) => {
    const i = feed.inbox as { items?: unknown[] } | null;
    return i?.items?.length ?? null;
  },
  ports: [
    "allow",
    "deny",
    "choose",
    "reply",
    "snooze",
    "copyrule",
    "focus",
    "approve",
    "retry",
    "resume",
    "open",
    "open_pr",
    "open_issue",
  ],
  select: (feed) => {
    const b = feed.inbox as
      | { items?: unknown[]; folded?: unknown[]; inhibited?: unknown[]; close?: unknown }
      | null;
    return {
      items: b?.items ?? [],
      folded: b?.folded ?? [],
      inhibited: b?.inhibited ?? [],
      close: b?.close ?? null,
    };
  },
  component: Inbox,
});

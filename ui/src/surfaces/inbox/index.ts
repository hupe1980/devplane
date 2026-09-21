import { register } from "../../lib/surfaces";
import Inbox from "./Inbox.svelte";

register({
  id: "inbox",
  title: "What needs you",
  band: "attention",
  order: 0,
  ports: ["allow", "deny", "choose", "reply", "snooze", "copyrule"],
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

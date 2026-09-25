// The board registers itself; nothing else names it.
import { register, phase } from "../../lib/surfaces";
import { bind, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import Board from "./Board.svelte";

bind({ surface: "global", combo: "g b", action: "go-board", label: "go to the sessions" });
onAction("go-board", () => {
  go("#board");
  return true;
});

register({
  id: "board",
  icon: "sessions",
  title: "Sessions",
  heading: "Sessions",
  band: "happening",
  order: 0,
  // `#board/<run>` selects that session, so a run opens here.
  holds: "run",
  // The machine's state, left of the status bar: sessions working, and any
  // that failed. A bucket with nothing in it is absent rather than a zero.
  status: (feed) => {
    const s = (feed.board as { summary?: { working?: number; failed?: number } } | null)?.summary;
    if (!s) return [];
    return [
      { n: s.working ?? 0, word: "working", icon: "sessions" },
      ...(s.failed ? [{ n: s.failed, word: "failed", icon: "alert", tone: "fail" as const }] : []),
    ];
  },
  count: (feed) => {
    const b = feed.board as { runs?: unknown[] } | null;
    return b?.runs?.length ?? null;
  },
  select: (feed, focus) => {
    const b = feed.board as {
      runs?: unknown[];
      summary?: unknown;
      coverage?: unknown;
      thresholds?: unknown;
      watching?: unknown;
    } | null;
    return {
      runs: b?.runs ?? [],
      summary: b?.summary,
      coverage: b?.coverage ?? null,
      // The host's configured threshold, the same one `devplane ls` uses.
      thresholds: b?.thresholds ?? null,
      // What this board can see at all, for the empty state.
      watching: b?.watching ?? null,
      // And whether it has seen anything yet: an empty list before the first
      // poll is not a quiet machine.
      focus,
      ...phase(feed),
    };
  },
  component: Board,
});

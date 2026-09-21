// The board registers itself. **Nothing else names it** — `loadSurfaces()`
// resolves this directory, so adding a surface is adding a directory.
import { register } from "../../lib/surfaces";
import Board from "./Board.svelte";

register({
  id: "board",
  title: "What is happening",
  band: "attention",
  order: 1,
  ports: ["attach", "focus"],
  select: (feed) => {
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
      // **The threshold comes from the daemon, never from the page.** It is
      // configurable, and a number hard-coded here would disagree with the one
      // `devplane ls` uses the moment somebody changes it — two surfaces
      // calling the same session crowded and fine.
      thresholds: b?.thresholds ?? null,
      // **What this board can see at all.** Without it the empty state is a
      // claim about the machine rather than about Devplane's own sight.
      watching: b?.watching ?? null,
    };
  },
  component: Board,
});

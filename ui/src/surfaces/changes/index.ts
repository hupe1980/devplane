// The changes surface registers itself. **Nothing else names it** — and that is
// the point this surface exists to prove: it was added as a directory, with no
// edit to any existing surface, no line in the shell and no route table.
import { register } from "../../lib/surfaces";
import Changes from "./Changes.svelte";

register({
  id: "changes",
  title: "What changed",
  band: "doing",
  order: 0,
  ports: [],
  select: (feed, focus) => {
    const b = feed.board as { work?: unknown[] } | null;
    return { works: b?.work ?? [], chosen: focus };
  },
  component: Changes,
});

// The changes surface registers itself. **Nothing else names it** — and that is
// the point this surface exists to prove: it was added as a directory, with no
// edit to any existing surface, no line in the shell and no route table.
import { register } from "../../lib/surfaces";
import Changes from "./Changes.svelte";

register({
  id: "changes",
  title: "Changes",
  heading: "What changed",
  band: "happening",
  order: 1,
  // **Not in the nav.** A diff is a detail of a Work, not a destination: top
  // level it was a picker with nothing in it on any machine that has never run
  // `devplane work start`, and a second picker beside the one *Finished work*
  // already has. It is reached from the Work it is about.
  nav: false,
  ports: [],
  select: (feed, focus) => {
    const b = feed.board as { work?: unknown[] } | null;
    return { works: b?.work ?? [], chosen: focus };
  },
  component: Changes,
});

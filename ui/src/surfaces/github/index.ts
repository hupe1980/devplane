import { register } from "../../lib/surfaces";
import Github from "./Github.svelte";

register({
  id: "github",
  title: "Issues and PRs",
  heading: "Issues and pull requests",
  band: "steering",
  order: 1,
  // **What needs you, not the total.** The nav has room for one number, and
  // *eleven open issues across six projects* is not the errand — *two of them
  // want you* is. Summed from the per-project counts the poll feed already
  // carries.
  count: (feed) => {
    const b = feed.board as { forge?: Record<string, { needs_you?: number }> } | null;
    const per = b?.forge;
    if (!per) return null;
    return Object.values(per).reduce((n, f) => n + (f.needs_you ?? 0), 0);
  },
  ports: ["github"],
  // The poll feed carries forge *counts*; the rows are a `gh` read on the
  // daemon's own schedule, and putting them in a two-second poll would send a
  // payload nobody is looking at on every board refresh.
  reads: ["/api/forge"],
  select: () => ({}),
  component: Github,
});

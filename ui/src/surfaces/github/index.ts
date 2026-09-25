import { register } from "../../lib/surfaces";
import Github from "./Github.svelte";

register({
  id: "github",
  icon: "forge",
  title: "Forge",
  heading: "Issues and pull requests",
  band: "steering",
  order: 1,
  // What needs you, not the total, from the feed's per-project counts.
  count: (feed) => {
    const b = feed.board as { forge?: Record<string, { needs_you?: number }> } | null;
    const per = b?.forge;
    if (!per) return null;
    return Object.values(per).reduce((n, f) => n + (f.needs_you ?? 0), 0);
  },
  // The rows are a `gh` read on the host's schedule, not part of the poll.
  reads: ["/api/forge"],
  // How many registered projects have a forge, for the empty state.
  select: (feed) => {
    const b = feed.board as { projects?: unknown[]; forge?: Record<string, unknown> } | null;
    if (!b?.projects) return {};
    return {
      coverage: { projects: b.projects.length, configured: Object.keys(b.forge ?? {}).length },
    };
  },
  component: Github,
});

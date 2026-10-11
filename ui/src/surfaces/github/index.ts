import { register } from "../../lib/surfaces";
import { bind, bindList, onAction } from "../../lib/keys";
import { go } from "../../lib/route";

bind({ surface: "global", combo: "g f", action: "go-forge", label: "go to the forge" });
bindList("github", "open the issue or pull request on GitHub");
onAction("go-forge", () => {
  go("#github");
  return true;
});

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
  // The rows are read from GitHub on the host's schedule, not part of the poll.
  reads: ["/api/forge"],
  // How many registered projects have a forge, for the empty state.
  select: (feed) => {
    const b = feed.board as { projects?: unknown[]; forge?: Record<string, unknown> } | null;
    if (!b?.projects) return {};
    return {
      coverage: { projects: b.projects.length, configured: Object.keys(b.forge ?? {}).length },
    };
  },
  page: () => import("./Github.svelte"),
});

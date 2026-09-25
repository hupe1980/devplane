import { bind, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import { register } from "../../lib/surfaces";
import Reports from "./Reports.svelte";

bind({ surface: "global", combo: "g r", action: "go-reports", label: "go to the reports" });
onAction("go-reports", () => {
  go("#reports");
  return true;
});

register({
  id: "reports",
  icon: "report",
  title: "Reports",
  heading: "Reports",
  band: "steering",
  order: 2,
  // Fetched on open, not polled; the inbox raises any that need a person.
  reads: ["/api/reports"],
  // The registered projects, for the form's two pickers and the narrowing.
  select: (feed) => {
    const b = feed.board as { projects?: Array<{ id: string; name: string }> } | null;
    return { projects: (b?.projects ?? []).map((p) => ({ id: p.id, name: p.name })) };
  },
  component: Reports,
});

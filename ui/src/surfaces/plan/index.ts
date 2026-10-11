import { bind, bindList, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import { register } from "../../lib/surfaces";

bind({ surface: "global", combo: "g s", action: "go-specs", label: "go to the specifications" });
bindList("plan", "open the specification as a tab of its own");
onAction("go-specs", () => {
  go("#plan");
  return true;
});

register({
  id: "plan",
  icon: "spec",
  title: "Specifications",
  heading: "Specifications",
  band: "happening",
  order: 3,
  // Read on open, not on the poll: it walks specification folders on disk.
  reads: ["/api/specs"],
  tab: (_feed, focus) => focus.split("/").filter(Boolean).pop() ?? "Specification",
  select: () => ({}),
  side: () => import("./List.svelte"),
  document: true,
  page: () => import("./Doc.svelte"),
});

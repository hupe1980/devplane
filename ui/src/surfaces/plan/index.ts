import { bind, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import { register } from "../../lib/surfaces";
import List from "./List.svelte";
import Doc from "./Doc.svelte";

bind({ surface: "global", combo: "g s", action: "go-specs", label: "go to the specifications" });
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
  side: List,
  component: Doc,
});

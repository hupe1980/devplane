import { register } from "../../lib/surfaces";
import Plan from "./Plan.svelte";

register({
  id: "plan",
  title: "Plans",
  heading: "What each project is working to",
  band: "happening",
  order: 3,
  ports: [],
  // Read on open rather than on the two-second poll: it walks every in-flight
  // work's specification folder on disk, which is not a thing to do twice a
  // second for a page nobody has open.
  reads: ["/api/specs"],
  select: () => ({}),
  component: Plan,
});

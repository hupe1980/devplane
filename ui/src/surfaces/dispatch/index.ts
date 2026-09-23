import { register } from "../../lib/surfaces";
import Dispatch from "./Dispatch.svelte";

register({
  id: "dispatch",
  title: "Start an agent",
  heading: "Start work",
  band: "steering",
  order: 0,
  ports: ["dispatch"],
  // The project list comes from the feed; what *would happen* is a preflight
  // the daemon computes on request, because it shells out to git per target.
  reads: ["/api/dispatch/preflight"],
  select: (feed) => {
    const b = feed.board as { projects?: Array<{ id: string; name: string }> } | null;
    return { projects: b?.projects ?? [] };
  },
  component: Dispatch,
});

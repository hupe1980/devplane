import { register } from "../../lib/surfaces";
import Dispatch from "./Dispatch.svelte";

register({
  id: "dispatch",
  title: "Start work",
  band: "doing",
  order: 1,
  ports: ["dispatch"],
  select: (feed) => {
    const b = feed.board as { projects?: Array<{ id: string; name: string }> } | null;
    return { projects: b?.projects ?? [] };
  },
  component: Dispatch,
});

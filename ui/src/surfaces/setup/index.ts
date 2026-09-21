import { register } from "../../lib/surfaces";
import Setup from "./Setup.svelte";

register({
  id: "setup",
  title: "What is configured",
  band: "project",
  order: 0,
  ports: ["setup"],
  select: () => ({}),
  component: Setup,
});

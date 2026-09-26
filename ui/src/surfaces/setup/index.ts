import { register } from "../../lib/surfaces";
import Setup from "./Setup.svelte";

register({
  id: "setup",
  icon: "settings",
  title: "Setup",
  heading: "Setup",
  band: "project",
  order: 0,
  // Fetched by the surface; declared so the route guard can see it.
  reads: ["/api/setup", "/api/github"],
  select: () => ({}),
  component: Setup,
});

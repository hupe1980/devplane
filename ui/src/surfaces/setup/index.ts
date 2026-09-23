import { register } from "../../lib/surfaces";
import Setup from "./Setup.svelte";

register({
  id: "setup",
  title: "Setup",
  heading: "What is configured",
  band: "project",
  order: 0,
  ports: ["setup"],
  // **This surface told every reader "Nothing is configured for this project
  // yet"** on machines with hooks installed and gates declared, because it had
  // `select: () => ({})` and fetched nothing. It reads the route now, and the
  // declaration is what a guard can see.
  reads: ["/api/setup"],
  select: () => ({}),
  component: Setup,
});

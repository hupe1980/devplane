import { register } from "../../lib/surfaces";
import { bind, onAction } from "../../lib/keys";
import { go } from "../../lib/route";

bind({ surface: "global", combo: "g ,", action: "go-setup", label: "go to the setup" });
onAction("go-setup", () => {
  go("#setup");
  return true;
});

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
  page: () => import("./Setup.svelte"),
});

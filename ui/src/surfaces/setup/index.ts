import { register, surfaces } from "../../lib/surfaces";
import { api } from "../../lib/api";
import { incomplete, skippedSteps, type FirstRun } from "./checklist";
import { bind, bindList, onAction } from "../../lib/keys";
import { go } from "../../lib/route";

// The project list: `j`/`k` move, Enter opens the inbox narrowed to it.
bindList("setup", "open the inbox narrowed to the project");
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
  // Declaring a project's gates opens here: `#<this>/declare/<project>`.
  holds: "gates",
  // Fetched by the surface; declared so the route guard can see it.
  reads: [
    "/api/setup",
    "/api/github",
    "/api/first-run",
    "/api/projects",
    "/api/projects/scan",
    "/api/projects/trust",
  ],
  // Opens first while the first-run checklist has a step neither done nor
  // skipped; an unreadable checklist never diverts the window.
  opensFirst: async () => {
    try {
      const r = await api<FirstRun>("/api/first-run");
      return Array.isArray(r?.steps) && incomplete(r.steps, skippedSteps());
    } catch {
      return false;
    }
  },
  select: () => ({ changeSurface: surfaces().find((s) => s.link === "change")?.id ?? "" }),
  page: () => import("./Setup.svelte"),
});

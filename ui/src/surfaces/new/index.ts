import { register, surfaces } from "../../lib/surfaces";
import { onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import New from "./New.svelte";

// Starting a change from anywhere. It floats over the current page, and
// closing it goes back there.
onAction("new-change", () => {
  if (location.hash.startsWith("#new")) return true;
  try {
    sessionStorage.setItem("vp-before-new", location.hash);
  } catch {
    /* closing goes to the landing page instead */
  }
  go("#new");
  return true;
});
onAction("leave", (surface) => {
  if (surface !== "new") return false;
  let back = "";
  try {
    back = sessionStorage.getItem("vp-before-new") ?? "";
  } catch {
    /* the landing page */
  }
  go(back);
  return true;
});

register({
  id: "new",
  title: "New change",
  heading: "New change",
  band: "steering",
  order: 0,
  nav: false,
  transient: true,
  reads: ["/api/projects", "/api/agents", "/api/specs", "/api/changes/preflight"],
  select: () => ({ changeSurface: surfaces().find((s) => s.link === "change")?.id ?? "" }),
  component: New,
});

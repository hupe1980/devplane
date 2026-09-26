import { register } from "../../lib/surfaces";
import { onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import Palette from "./Palette.svelte";

/// Where the palette was opened from, so `Esc` goes back there.
let from = "";

// `Mod+k` is bound globally in the registry; this is what answers it.
onAction("open-palette", () => {
  if (location.hash.startsWith("#palette")) return true;
  from = location.hash;
  go("#palette");
  return true;
});
onAction("leave", (surface) => {
  if (surface !== "palette") return false;
  go(from || "");
  from = "";
  return true;
});
// No list keys in the registry: focus stays in the palette's field, which
// takes ↑, ↓ and Enter itself, so a binding here could never fire.

register({
  id: "palette",
  title: "Palette",
  heading: "Everything, by name",
  band: "happening",
  order: 9,
  // Reached by the key, never listed.
  nav: false,
  transient: true,
  reads: ["/api/changes", "/api/projects"],
  select: () => ({ from }),
  component: Palette,
});

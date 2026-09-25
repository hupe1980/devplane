// The workbench frame's own keys, bound through the same `bind()` as every
// surface so a collision fails the build.
import { bind } from "../lib/keys";

bind({ surface: "global", combo: "Mod+b", action: "toggle-side", label: "show or hide the sidebar" });
bind({ surface: "global", combo: "Mod+j", action: "toggle-panel", label: "show or hide the activity panel" });
bind({ surface: "global", combo: "Alt+w", action: "close-tab", label: "close the tab" });
bind({ surface: "global", combo: "Alt+]", action: "next-tab", label: "next tab" });
bind({ surface: "global", combo: "Alt+[", action: "prev-tab", label: "previous tab" });
bind({ surface: "global", combo: "Alt+n", action: "new-change", label: "start a new change" });

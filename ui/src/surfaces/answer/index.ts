import { register } from "../../lib/surfaces";
import { bindList } from "../../lib/keys";
import { bindItemKeys } from "../inbox/keys";

// The same keys as the inbox, from the same list: this window is the inbox's
// one item at a time. `j`/`k` move between the items waiting.
bindList("answer", "move into the item (Tab then reaches its controls)");
bindItemKeys("answer");

register({
  id: "answer",
  title: "Answer",
  heading: "The one thing that needs you",
  band: "attention",
  order: 9,
  // The shortcut's window, not a place in the nav: it shows the topmost
  // waiting item and nothing else, in a frame 480 pixels wide.
  nav: false,
  bare: true,
  reads: ["/api/inbox"],
  select: () => ({}),
  page: () => import("./Answer.svelte"),
});

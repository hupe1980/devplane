import { register } from "../../lib/surfaces";
import Answer from "./Answer.svelte";

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
  component: Answer,
});

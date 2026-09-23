import { register } from "../../lib/surfaces";
import Why from "./Why.svelte";

register({
  id: "why",
  title: "Decisions",
  heading: "Why this was decided",
  band: "attention",
  order: 1,
  ports: ["why"],
  // **Opened about something, or it has nothing to say.** It rendered "open a
  // row and this shows what was decided" on every visit, with no row to open
  // and nothing able to give it one.
  select: (_feed, focus) => ({ about: focus }),
  component: Why,
});

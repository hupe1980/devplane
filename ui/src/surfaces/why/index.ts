import { register } from "../../lib/surfaces";
import Why from "./Why.svelte";

register({
  id: "why",
  title: "Why this is here",
  band: "attention",
  order: 3,
  ports: ["why"],
  // **Opened about something, or it has nothing to say.** It rendered "open a
  // row and this shows what was decided" on every visit, with no row to open
  // and nothing able to give it one.
  select: (_feed, focus) => ({ about: focus }),
  component: Why,
});

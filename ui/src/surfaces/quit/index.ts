import { register } from "../../lib/surfaces";
import Quit from "./Quit.svelte";

register({
  id: "quit",
  title: "Quit",
  heading: "Quit Devplane?",
  band: "project",
  order: 9,
  // The quit question, raised in the answer window by ⌘Q and the tray. Not
  // a place: it says what quitting stops and offers the two answers.
  nav: false,
  bare: true,
  reads: ["/api/quitting"],
  select: () => ({}),
  component: Quit,
});

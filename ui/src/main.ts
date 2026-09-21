// The one entry point. Everything else registers itself.
//
// **The tokens are imported here and nowhere else.** They were carried over
// from the hand-written page on the day the scaffold landed and then imported
// by nothing, so every `var(--dim)` in every surface resolved to nothing —
// twenty-seven uses in the built stylesheet and not one definition. The page
// rendered, which is why it went unnoticed: unstyled text is still text.
import "./tokens.css";
import "./base.css";
import { mount } from "svelte";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("no #app to mount into");

export default mount(App, { target });

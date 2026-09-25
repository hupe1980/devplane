// The one entry point. Everything else registers itself. The stylesheets are
// imported here and nowhere else; a guard checks they are.
import "./tokens.css";
import "./base.css";
import { mount } from "svelte";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("no #app to mount into");

export default mount(App, { target });

// The one entry point. Everything else registers itself. The stylesheets are
// imported here and nowhere else; a guard checks they are.
import "./tokens.css";
import "./base.css";
import { mount } from "svelte";
import App from "./App.svelte";
import Elsewhere from "./Elsewhere.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("no #app to mount into");

// The app's second start, on the bundled page: a host it does not own already
// serves this home. Nothing else mounts, so nothing reads the API.
const elsewhere = Number(new URLSearchParams(location.search).get("elsewhere"));

export default Number.isInteger(elsewhere) && elsewhere > 0
  ? mount(Elsewhere, { target, props: { port: elsewhere } })
  : mount(App, { target });

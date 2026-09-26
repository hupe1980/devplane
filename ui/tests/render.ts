// Renders every surface with Svelte's server renderer and asserts what came
// out. The sections are separate files by subject; this imports them in order
// and reports. See `harness.ts` for why there is no test runner.
import "./render/registry";
import "./render/board";
import "./render/inbox";
import "./render/change";
import "./render/review";
import "./render/honest";
import "./render/controls";
import "./render/places";
import "./render/behaviour";
import { finish } from "./harness";

finish();

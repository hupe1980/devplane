// The build check's entry: importing every surface runs every `bind()`, which
// throws on a collision and stops the build.
import { loadSurfaces } from "../src/lib/load";
import "../src/shell/keys";
import { all } from "../src/lib/keys";

loadSurfaces();

export const bindings = all();

// Fails the build on a key collision. `npm run build` runs this first: it
// builds every surface with the same Vite config and glob, and imports the
// result, where `bind()` throws naming the colliding pair. Output: `.ssr/`.

import { build } from "vite";
import { fileURLToPath, pathToFileURL } from "node:url";
import { resolve } from "node:path";

// `fileURLToPath`, not `URL.pathname`: on Windows the pathname is `/D:/…`, and
// resolving it yields `D:\D:\…`.
const here = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const out = resolve(here, ".ssr");

await build({
  root: here,
  configFile: resolve(here, "vite.config.ts"),
  logLevel: "error",
  build: {
    ssr: resolve(here, "scripts/keys-entry.ts"),
    outDir: out,
    emptyOutDir: false,
    sourcemap: false,
    rollupOptions: { output: { entryFileNames: "keys.js" } },
  },
});

let mod;
try {
  mod = await import(pathToFileURL(resolve(out, "keys.js")).href + `?t=${Date.now()}`);
} catch (e) {
  console.error(`check-keys: ${e instanceof Error ? e.message : String(e)}`);
  process.exit(1);
}

const bindings = mod.bindings;
const scopes = new Set(bindings.map((b) => b.surface));
if (bindings.length === 0) {
  console.error("check-keys: no bindings at all — the registry is not being read");
  process.exit(1);
}
// The rule again over the finished list, in case `bind()` stopped checking.
const seen = new Map();
for (const b of bindings) {
  const key = `${b.surface}\u0000${b.combo}`;
  if (seen.has(key)) {
    console.error(`check-keys: ${b.combo} is bound twice in ${b.surface}: "${seen.get(key).label}" and "${b.label}"`);
    process.exit(1);
  }
  seen.set(key, b);
  if (b.surface !== "global" && seen.has(`global\u0000${b.combo}`)) {
    console.error(`check-keys: ${b.surface} rebinds the global ${b.combo}: "${b.label}"`);
    process.exit(1);
  }
  if (!b.label.trim()) {
    console.error(`check-keys: ${b.surface} ${b.combo} has no label`);
    process.exit(1);
  }
}
console.log(`check-keys: ok (${bindings.length} bindings across ${scopes.size} scopes)`);

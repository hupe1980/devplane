// Fails the build on a key the help sheet would be lying about. `npm run
// build` runs this first: it builds every surface with the same Vite config
// and glob, and imports the result, where `bind()` throws naming a colliding
// pair. Then, over the finished list: a collision, a binding with no label, a
// binding whose action nothing answers ("a key nothing reads"), a list key on
// a surface with no cursor, a combo no keyboard event can spell, and a single
// key that swallows the first half of a chord. Output: `.ssr/`.

import { build } from "vite";
import { fileURLToPath, pathToFileURL } from "node:url";
import { resolve, join } from "node:path";
import { readFileSync, readdirSync } from "node:fs";

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

/** @typedef {{ surface: string; combo: string; action: string; label: string }} Binding */
/** @type {Binding[]} */
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

/** @type {string[]} */
const problems = [];

// ── every action has an answer ─────────────────────────────────────────────
// Listeners subscribe inside components, which a server build never mounts,
// so the answers are read from the source: every `onAction("<name>"` call,
// and the list actions `lib/cursor.ts` answers for each `cursor({ scope })`.
/** @param {string} dir @returns {string[]} */
function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? files(join(dir, e.name)) : /\.(ts|svelte)$/.test(e.name) ? [join(dir, e.name)] : [],
  );
}
const strip = (/** @type {string} */ t) => t.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/[^\n]*/g, "$1").replace(/<!--[\s\S]*?-->/g, "");
const sources = files(resolve(here, "src")).map((f) => [f, strip(readFileSync(f, "utf8"))]);
/** @type {Set<string>} */
const answered = new Set();
for (const [, text] of sources) for (const m of text.matchAll(/onAction\(\s*["']([\w-]+)["']/g)) answered.add(m[1]);
const cursorSrc = sources.find(([f]) => f.endsWith(join("lib", "cursor.ts")))?.[1] ?? "";
const listActions = new Set([...cursorSrc.matchAll(/\bon\(\s*"([\w-]+)"/g)].map((m) => m[1]));
/** @type {Set<string>} */
const cursorScopes = new Set();
for (const [, text] of sources) for (const m of text.matchAll(/cursor\(\s*\{\s*scope:\s*["']([\w-]+)["']/g)) cursorScopes.add(m[1]);
for (const b of bindings) {
  if (listActions.has(b.action)) {
    if (!cursorScopes.has(b.surface))
      problems.push(`${b.surface} binds the list key ${b.combo} (${b.action}), and no \`cursor({ scope: "${b.surface}" })\` answers it`);
  } else if (!answered.has(b.action)) {
    problems.push(`${b.surface} binds ${b.combo} to "${b.action}", and nothing calls onAction("${b.action}") — a key nothing reads`);
  }
}

// ── every combo can be spelled by a key event (`combo()` in lib/keys.ts) ───
const NAMED = new Set(["Esc", "Enter", "Space", "Tab", "Up", "Down", "Left", "Right", "Backspace", "Delete", "Home", "End", "PageUp", "PageDown"]);
const ALT_KEYS = /^([a-z0-9]|[[\]\-=,./;'`\\])$/;
for (const b of bindings) {
  for (const chord of b.combo.split(" ")) {
    const parts = chord.split("+");
    const key = parts.pop() ?? "";
    const why =
      parts.some((m) => !["Mod", "Alt", "Shift"].includes(m))
        ? `an unknown modifier`
        : key.length > 1 && !NAMED.has(key) && !/^F\d{1,2}$/.test(key)
          ? `"${key}" is not a key name combo() produces`
          : parts.includes("Shift") && key.length === 1
            ? `Shift is spelled by the character itself ("G", "?"), never as Shift+${key}`
            : parts.includes("Alt") && !ALT_KEYS.test(key)
              ? `with Alt the key is read from e.code, which spells lowercase letters, digits and US punctuation`
              : (parts.includes("Mod") || parts.includes("Alt")) && /^[A-Z]$/.test(key)
                ? `a chord with a modifier is spelled with a lowercase letter`
                : "";
    if (why) problems.push(`${b.surface} ${b.combo} can never fire: ${why}`);
  }
}

// ── a single key does not swallow a chord's first half ─────────────────────
for (const chordB of bindings.filter((b) => b.combo.includes(" "))) {
  const first = chordB.combo.split(" ")[0];
  const shadow = bindings.find(
    (b) =>
      b.combo === first &&
      (b.surface === chordB.surface || b.surface === "global" || chordB.surface === "global"),
  );
  if (shadow)
    problems.push(`${shadow.surface} ${first} ("${shadow.label}") takes the key that starts ${chordB.surface} ${chordB.combo} ("${chordB.label}")`);
}

if (problems.length) {
  for (const p of problems) console.error(`check-keys: ${p}`);
  process.exit(1);
}
console.log(`check-keys: ok (${bindings.length} bindings across ${scopes.size} scopes, every one answered)`);

// The render harness's tools, shared by every section under `render/`. No
// test runner: Svelte's server renderer returns strings and the assertions are
// `if` statements. Browser behaviour (a sort, a key) is tested by calling the
// function the component exports for it; nothing here pretends to click.
import { readFileSync, readdirSync } from "node:fs";
import { render } from "svelte/server";

let failures = 0;

export const fail = (m: string): void => {
  console.error("ui_surfaces: " + m);
  failures += 1;
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const html = (c: any, props: Record<string, unknown>): string => render(c, { props }).body;

/// A source file with its comments (whole spans) removed — the only way this
/// harness reads a source, so a check never matches its own explanation.
/// Paths are relative to the bundled `.ssr/app.js`: `../src/` for the
/// interface, `../../src/` for the host.
export function source(rel: string): string {
  return readFileSync(new URL(rel, import.meta.url), "utf8")
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/\/\/\/?[^\n]*/g, "");
}

/// Every file under a directory, relative in the same way `source` takes it.
export function walk(dir: string): string[] {
  return readdirSync(new URL(dir, import.meta.url), { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? walk(`${dir}${e.name}/`) : [`${dir}${e.name}`],
  );
}

/// Markup with the stylesheet and inline styles removed, so a CSS width is not
/// read as a figure.
export const bare = (markup: string): string =>
  markup.replace(/<style[\s\S]*?<\/style>/g, "").replace(/style="[^"]*"/g, "");

/// What a person reads: the markup's text, entities for `<`, `>` and `&` left
/// as the renderer wrote them.
export const visible = (markup: string): string => bare(markup).replace(/<[^>]*>/g, " ");

export function finish(): void {
  if (failures > 0) {
    console.error(`ui_surfaces: ${failures} failure(s)`);
    process.exit(1);
  }
  console.log("ui_surfaces: ok");
}

// Resolves the surface directory, and does nothing else.
//
// **This is its own module because putting the glob in `surfaces.ts` was a
// dependency cycle**, and the client build only worked by accident of
// ordering. `import.meta.glob(..., { eager: true })` is rewritten to static
// imports at the top of the module that contains it — so `surfaces.ts` imported
// `board/index.ts`, which imported `surfaces.ts`, and `register()` ran before
// `const registry = new Map()` had executed. It threw the moment the server
// renderer imported things in a different order.
//
// A module with one job and no imports of its own cannot be in a cycle: the
// registry does not know this file exists.
//
// Eager rather than lazy, because a surface that loaded on first use would
// leave the palette and the `?` sheet incomplete until somebody had already
// visited the thing they were looking for.
export function loadSurfaces(): void {
  import.meta.glob("../surfaces/*/index.ts", { eager: true });
}

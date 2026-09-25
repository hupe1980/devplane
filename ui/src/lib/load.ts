// Resolves the surface directory, and does nothing else. Its own module so the
// eager glob (rewritten to static imports) cannot form a cycle with
// `surfaces.ts`. Eager, so the palette and `?` sheet list every surface.
export function loadSurfaces(): void {
  import.meta.glob("../surfaces/*/index.ts", { eager: true });
}

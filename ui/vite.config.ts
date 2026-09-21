import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The one thing this config needs from Node, declared rather than depended on.
// `@types/node` is 2 MB of ambient declarations to type a single environment
// read, in a repository that counts its dependencies and publishes the number.
declare const process: { env: Record<string, string | undefined> };

// Devplane's interface, built to a bundle the Rust binary embeds at compile
// time. Three settings here are not preferences — each is a property the
// product argues for elsewhere, enforced at the one place that can enforce it.
export default defineConfig({
  plugins: [svelte()],

  build: {
    outDir: "dist",
    emptyOutDir: true,

    // **The output stays readable.** Not developer convenience: this is a
    // product about being able to see what was decided on your machine, and the
    // interface has always been the one artefact a person could read without
    // this repository — over a tunnel, with `curl`, on a machine that has never
    // built it. Shipping an opaque bundle would make the accountability tool
    // the least accountable thing in it.
    //
    // The cost is bytes, and a compiler-first framework has the room: readable
    // output at a few kilobytes is still an order of magnitude under an
    // unreadable mainstream runtime.
    minify: false,

    // No source map is *required* to follow the output — that is what `minify:
    // false` buys. One is emitted anyway because it costs nothing at rest and
    // helps whoever is debugging; the readability check asserts against the
    // bundle, never against the map.
    sourcemap: true,

    // **Everything inlined into one document where it fits.** The binary
    // embeds what is built here, and a second file is a second thing to embed,
    // serve and keep in step. 100 KB is above anything this interface should
    // produce; passing it is a signal rather than a limit.
    assetsInlineLimit: 100_000,

    rollupOptions: {
      output: {
        // Stable names, because the Rust side embeds them by path and a
        // content hash would mean regenerating that list on every build.
        entryFileNames: "app.js",
        chunkFileNames: "[name].js",
        assetFileNames: "[name][extname]",
      },
    },

    // The floor the whole rebuild is measured against is the current page's
    // 40 036 gzipped bytes. Warn well under it, so the number is noticed while
    // it is still a choice.
    chunkSizeWarningLimit: 60,
  },

  // **Nothing is fetched from outside the machine, in any build, for any
  // asset** — including a font. A control plane whose own interface phones
  // somewhere is not one. There is no CDN, no font URL and no analytics here,
  // and `tests/ui_contract.rs` asserts the absence against the served bytes
  // rather than trusting this comment.
  server: {
    // Dev only: the daemon is on loopback and serves the API the app reads.
    proxy: {
      "/api": daemon(),
    },
  },

  // **`preview` exists so the rebuild can be photographed before it is
  // served.** The screenshot script drives the page compiled into the binary,
  // which is the interface being replaced — so until the switch there was no
  // way to look at the rebuilt one with real data, and the three tasks gating
  // the switch are all *somebody looks at it*.
  //
  // This is not a second served interface, which the switch refuses. Nothing
  // ships it and the daemon does not know it exists; it is a static server over
  // `dist/` with the API proxied at a port the script passes in.
  preview: {
    proxy: {
      "/api": daemon(),
    },
  },
});

/// Where the daemon is, for the two dev servers that proxy to it.
///
/// A throwaway daemon picks a free port and writes it to `daemon.json`, so the
/// screenshot script reads it there and passes it in. The fallback is the
/// default port, which is what `npm run dev` against your own daemon wants.
function daemon(): string {
  return `http://127.0.0.1:${process.env.DEVPLANE_PORT ?? 47831}`;
}

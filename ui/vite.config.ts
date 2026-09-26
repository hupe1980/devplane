import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The one thing this config needs from Node, declared rather than pulling in
// `@types/node`.
declare const process: { env: Record<string, string | undefined> };

// Devplane's interface, built to a bundle the Rust binary embeds at compile time.
export default defineConfig({
  plugins: [svelte(), ...(process.env.DEVPLANE_FIXTURES ? [fixtures()] : [])],

  build: {
    outDir: "dist",
    emptyOutDir: true,

    // The served output stays readable without this repository — over a
    // tunnel, with `curl`. `tests/ui_bundle.rs` checks the bytes.
    minify: false,

    // Emitted for debugging and never embedded; readability never relies on it.
    sourcemap: true,

    // Inline assets where they fit, so there are fewer files to embed and serve.
    assetsInlineLimit: 100_000,

    rollupOptions: {
      output: {
        // Stable names: the Rust side embeds them by path.
        entryFileNames: "app.js",
        chunkFileNames: "[name].js",
        assetFileNames: "[name][extname]",
      },
    },

    // Warn early, so growth is noticed while it is still a choice.
    chunkSizeWarningLimit: 60,
  },

  // Nothing is fetched from outside the machine — no CDN, font or analytics.
  // `tests/ui_bundle.rs` asserts it against the served bytes.
  server: {
    // Dev only: the host is on loopback and serves the API the app reads.
    proxy: {
      "/api": host(),
    },
  },

  // A static server over `dist/` for screenshots, with the API proxied. Nothing
  // ships it.
  preview: {
    proxy: {
      "/api": host(),
    },
  },
});

/// Where the host is, for the dev servers that proxy to it: `DEVPLANE_PORT`
/// (set by the screenshot script), else the default port.
function host(): string {
  return `http://127.0.0.1:${process.env.DEVPLANE_PORT ?? 47831}`;
}

/// `npm run dev:fixtures`: answers `/api/*` from `ui/fixtures/` (recorded by
/// `scripts/capture-fixtures.sh`), with no host running. A missing recording is
/// a 404; a write answers `{}` unless a refusal of it was recorded. Dev only.
function fixtures() {
  return {
    name: "devplane-fixtures",
    configureServer(server: { middlewares: { use: (f: (req: Req, res: Res, next: () => void) => void) => void } }) {
      server.middlewares.use(async (req, res, next) => {
        const url = new URL(req.url ?? "/", "http://x");
        if (!url.pathname.startsWith("/api/")) return next();
        res.setHeader("content-type", "application/json");
        const parts = url.pathname.slice(5).split("/");
        const name =
          parts.length === 1
            ? parts[0]
            : parts.length === 2
              ? `${parts[0].replace(/s$/, "")}-${parts[1]}`
              : `${parts[2]}-${parts[1]}`;
        // @ts-expect-error — `@types/node` is not installed, on purpose (see the top of this file).
        const { readFile } = await import("node:fs/promises");
        // A write answers `{}`, unless its refusal was recorded (an offer the
        // host refused), which answers as the host did: 409 with its body.
        if (req.method !== "GET") {
          try {
            const body = await readFile(new URL(`./fixtures/${name}.json`, import.meta.url));
            if (String(body).includes('"refused"')) res.statusCode = 409;
            return res.end(body);
          } catch {
            return res.end("{}");
          }
        }
        try {
          res.end(await readFile(new URL(`./fixtures/${name}.json`, import.meta.url)));
        } catch {
          res.statusCode = 404;
          res.end(JSON.stringify({ error: `no recording for ${url.pathname}` }));
        }
      });
    },
  };
}
type Req = { url?: string; method?: string };
type Res = { setHeader: (k: string, v: string) => void; end: (b?: string | Uint8Array) => void; statusCode: number };

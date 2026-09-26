// The little of Node the harness and the scripts use, declared rather than
// pulling in `@types/node` (see `vite.config.ts`), so `npm run check` can type
// `tests/` and `scripts/` too.
declare module "node:fs" {
  export function readFileSync(path: string | URL, encoding: "utf8"): string;
  export function readdirSync(
    path: string | URL,
    options: { withFileTypes: true },
  ): Array<{ name: string; isDirectory(): boolean }>;
  export function readdirSync(path: string | URL): string[];
}
declare module "node:url" {
  export function fileURLToPath(url: string | URL): string;
  export function pathToFileURL(path: string): URL;
}
declare module "node:path" {
  export function resolve(...parts: string[]): string;
  export function join(...parts: string[]): string;
}
declare const process: { exit(code?: number): never; env: Record<string, string | undefined>; argv: string[] };

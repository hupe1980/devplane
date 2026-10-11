# Contributing

The most useful contribution is a precise report; the second is a test.

## Reports that matter most

1. **A permission verdict that was wrong**: a prohibition that did not fire, or a grant that allowed
   more than its rule. Use the *gate widening* issue template with the exact call and the exact rule.
2. **A vendor release that moved something.** Use the *vendor drift* template and quote the
   changelog row.

## Prerequisites

- Rust 1.94 or later, [`just`](https://just.systems) and bash.
- Node 22 or later, for the interface in `ui/`.
- [Zola](https://www.getzola.org) 0.23, for the site.
- On Linux, `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`:
  clippy runs with every feature, which compiles the window.

## Before a pull request

```sh
just check                   # fmt, clippy, build, test, wire-check — what CI runs
just ui-build ui-check wire  # the interface, its type check, the generated wire types
just verify                  # check, then the dependency count and the site build and link check
just bars                    # the UX bars, each against its budget (needs a Playwright Chromium)
```

CI fails when `ui/dist` or `ui/src/wire` differs from what those recipes produce, so commit any diff
they leave. `just notes` checks the maintainer's notes and prints "skipped" in a checkout without
them; contributors need nothing from it.

A change to the permission layer needs a test that asserts **both** the verdict and the rule that
produced it. A dependency bump is a reviewed change: agents and packages are pinned on purpose.

## The interface

`ui/` is a Svelte project built to a bundle the binary embeds at compile time.

```sh
cd ui && npm install
npm run build         # → ui/dist/, embedded into the binary
npm run dev           # a dev server that proxies /api to a running host (`devplane up --no-browser`)
npm run check         # svelte-check over TypeScript and every component
npm run render        # renders every surface and asserts on the result
npm run dev:fixtures  # the interface over recorded host responses, no host needed (POSIX shells)
```

- **`ui/dist/` is committed**, because `cargo publish` packages what is in git.
- **`ui/fixtures/` are recorded, never hand-written**: `bash scripts/capture-fixtures.sh` seeds a real
  host and saves what it serves; `just make-board` photographs the same seed into `site/static/`.
- **`ui/src/wire/` is generated** from the Rust types (`ts-rs`) by `just wire`; commit new files too.
- **The built output stays readable** (`minify: false`), and **nothing is fetched from outside the
  machine**, fonts included.
- A surface is a directory under `ui/src/surfaces/`: an `index.ts` that registers itself and binds its
  keys, plus its components. Keys go through `ui/src/lib/keys.ts`; `npm run build` fails on a key
  bound twice in one scope or a surface rebinding a global.
- **`cargo package` shares the target directory** and can leave a packaged interface a later build
  reuses: `rm -rf target/package` after packaging.

## The window

`just app` runs `cargo run --features app -- up --window`. The feature is off by default; on Linux it
needs the packages under *Prerequisites*. `cargo test --features app --test app` drives the app's host
without a window; `scripts/make-icon.py` regenerates `icons/`.

## Out of scope

A model deciding a permission or a gate result; a cloud relay or account; a second rule language;
parsing transcript JSONL on the critical path; a React or WASM interface.

## Licence

MIT OR Apache-2.0, at your option. By contributing you agree your contribution is licensed the same
way.

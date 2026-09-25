# Contributing

One person maintains Devplane. The most useful contribution is a precise report; the second is a
test.

## Reports that matter most

1. **A prohibition that did not fire.** Use the *gate widening* issue template with the exact call
   and the exact rule. (Devplane never approves a call, so there is no allow-side report.)
2. **A vendor release that moved something.** Claude Code, Copilot and Codex change weekly. Use the
   *vendor drift* template and quote the changelog row.

## Before a pull request

```sh
just check          # fmt, clippy, build, test — what CI runs
just verify         # check, then deps (dependency count), site-build and site-check (Zola links)
just notes          # checks over the maintainer's gitignored notes; says "skipped" where absent
```

Features are specified with [GitHub Spec Kit](https://github.com/github/spec-kit) before they are
built. Those working files are not published; every behaviour they ask for is a test, and
`just verify` runs all of them.

A change to the permission layer needs a test that asserts **both** the verdict and the rule that
produced it. A dependency bump is a reviewed change: agents and packages are pinned on purpose.

## The interface

`ui/` is a Svelte project built to a bundle the binary embeds at compile time.

```sh
cd ui && npm install     # Node 22+
npm run build            # → ui/dist/, embedded into the binary
npm run dev              # a dev server that proxies /api to a running host (`devplane serve`)
npm run check            # svelte-check over TypeScript and every component
npm run render           # renders every surface and asserts on the result
npm run dev:fixtures     # the interface over recorded host responses, no host needed
```

- **`ui/dist/` is committed**, because `cargo publish` packages what is in git. Change a surface, run
  `npm run build`, commit the result. CI rebuilds it and fails if `ui/dist` differs.
- **`ui/fixtures/` are recorded, never hand-written**: `bash scripts/capture-fixtures.sh` seeds a real
  host and saves what it serves. `bash scripts/make-board.sh` photographs the same seed into
  `site/static/`.
- **`ui/src/wire/` is generated** from the Rust types (`ts-rs`). After changing a type that crosses
  the API, run `TS_RS_EXPORT_DIR=ui/src cargo test --features typescript export_bindings` and commit
  the result.
- **The built output stays readable** (`minify: false`), and **nothing is fetched from outside the
  machine**, fonts included.
- A surface is a directory under `ui/src/surfaces/`: an `index.ts` that registers itself and binds its
  keys, plus its components. The frame is `ui/src/shell/`. Keys go through `ui/src/lib/keys.ts`;
  `npm run build` runs `scripts/check-keys.mjs` first and fails on a key bound twice in one scope or a
  surface rebinding a global. `tests/ui_bundle.rs` holds the interface to the API and the bundle.
- **`cargo package` shares the target directory**, and its verification build embeds the packaged
  interface, which a later `cargo build` can reuse. `rm -rf target/package` after packaging, or set
  `CARGO_TARGET_DIR` elsewhere.
- TypeScript is pinned at 5.9 because `svelte-check` does not accept 7.

## The window

```sh
just app                 # cargo run --features app -- app
```

The app is the cargo feature `app`, off by default. On macOS it needs no system packages; on Linux
install `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev` first
(`cargo clippy --all-features` compiles the app). `tests/app.rs` drives the app's host without a
window (`cargo test --features app --test app`); `scripts/make-icon.py` regenerates `icons/`.

## Out of scope

A model deciding a permission or a gate result; a cloud relay or account; a second rule language;
parsing transcript JSONL on the critical path; a React or WASM interface.

## Licence

MIT OR Apache-2.0, at your option. By contributing you agree your contribution is licensed the same way.

# Contributing

Devplane is early and one person maintains it. The most useful contribution is a precise report,
and the second most useful is a test.

## Reports that matter most

1. **A prohibition that did not fire.** Use the *gate widening* issue template. Every defect of this
   kind found so far was silent, so an exact call and an exact rule is worth more than a feature
   request. (Devplane never *approves* a call, so there is no allow-side report to make.)
2. **A vendor release that moved something.** Claude Code, Copilot and Codex change weekly. Use the
   *vendor drift* template and quote the changelog row.

## Before a pull request

```sh
just check          # fmt, clippy, build, test — what CI runs
just verify         # plus the claim ledger and the dependency count; needs `just reference` once
```

## Where a change is planned

A feature is specified before it is built, with
[GitHub Spec Kit](https://github.com/github/spec-kit) — requirements with stable ids, a plan checked
against a written constitution, then a task list. Those working files are not published — they are
the maintainer's, and a reader would have neither the folder nor the identifiers it cites.

**What reaches you instead is the result.** Every behaviour the specification asked for is a test
that names it, and `just verify` runs all of them. If you want to know what a feature must do, the
tests are the answer that cannot go stale.

Every behaviour change to the permission layer needs a test that asserts **both** the verdict and
the rule that produced it; a right answer with a wrong reason is a bug here. `Verdict` has no
`Allow` variant and will not be given one. A dependency bump is a
reviewed change: agents and packages are pinned on purpose.

## The interface

**`ui/` is a Svelte project**, being ported from the single hand-written page the binary still
serves. The reason for the build step is that the interfaces this product now needs are editors
rather than lists.

```sh
cd ui && npm install     # Node 22+
npm run build            # → ui/dist/, embedded into the binary at compile time
npm run dev              # a dev server that proxies /api to a running daemon
npm run check            # svelte-check, over TypeScript and every component
```

**`ui/dist/` is committed**, because `cargo publish` packages what is in git. Change a surface, run
`npm run build`, and commit the result — CI rebuilds it and fails if it differs. The build is
byte-reproducible, so it only differs when it is stale.

**Two properties are not preferences, and both are enforced rather than requested.**

**The built output stays readable** — `minify: false`. This is a product about being able to see what
was decided on your machine, and the interface has always been the one artefact a person could read
without this repository: over a tunnel, with `curl`, on a machine that has never built it. Shipping
an opaque bundle would make the accountability tool the least accountable thing in it.

**Nothing is fetched from outside the machine, in any build, for any asset — including a font.** A
control plane whose own interface phones somewhere is not one.

`tests/ui_bundle.rs` holds the interface to the API and to the bundle it ships;
`ui/tests/render.ts` renders every surface and asserts on the result. The second needs `node`, so
install it before trusting a green run of the interface tests.

**`cargo package` shares the target directory**, and its verification build embeds the interface
from the packaged copy. A `cargo build` afterwards can reuse that build-script output and produce a
binary carrying the *packaged* bundle rather than `ui/dist` — a stale interface, silently. `rm -rf
target/package` after packaging, or package with `CARGO_TARGET_DIR` set elsewhere.

**Versions are pinned on purpose**, including the toolchain. TypeScript is held at 5.9 because
`svelte-check` does not accept 7 yet; the registry's `latest` is not usable here.

A surface is a directory under `ui/src/surfaces/`: an `index.ts` that registers itself and a
component. Nothing else names it, so adding one is not a merge conflict.

## Scope

Things that will not be merged, so nobody spends a weekend on them: a model deciding a permission
or a gate result; a cloud relay or account; a second rule language; parsing transcript JSONL on the
critical path; a React or WASM interface, which were measured against and rejected on weight. The public docs explain the reasoning:
<https://hupe1980.github.io/devplane/docs/decisions/>.

## Licence

MIT OR Apache-2.0, at your option. By contributing you agree your contribution is licensed the same way.

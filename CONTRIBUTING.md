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

**Two properties are not preferences, and both are enforced rather than requested.**

**The built output stays readable** — `minify: false`. This is a product about being able to see what
was decided on your machine, and the interface has always been the one artefact a person could read
without this repository: over a tunnel, with `curl`, on a machine that has never built it. Shipping
an opaque bundle would make the accountability tool the least accountable thing in it.

**Nothing is fetched from outside the machine, in any build, for any asset — including a font.** A
control plane whose own interface phones somewhere is not one.

`tests/ui_contract.rs` holds the interface to the API, to escaping every value it prints, and to
rendering at all. That last one needs `node` and **skips silently without it**, so install node
before trusting a green run of the interface tests.

**Versions are pinned on purpose**, including the toolchain. TypeScript is held at 5.9 because
`svelte-check` does not accept 7 yet; the registry's `latest` is not usable here.

`ui/legacy.html` is the page being replaced. It is still what the binary serves, and it goes in the
same change that serves the bundle — one switch, revertible in one move. Serving both at once is
refused: two interfaces that must not diverge is a worse problem than one.

## Scope

Things that will not be merged, so nobody spends a weekend on them: a model deciding a permission
or a gate result; a cloud relay or account; a second rule language; parsing transcript JSONL on the
critical path; a React or WASM interface, which were measured against and rejected on weight. The public docs explain the reasoning:
<https://hupe1980.github.io/devplane/docs/decisions/>.

## Licence

MIT OR Apache-2.0, at your option. By contributing you agree your contribution is licensed the same way.

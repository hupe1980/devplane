# Contributing

Vibeplane is early and one person maintains it. The most useful contribution is a precise report,
and the second most useful is a test.

## Reports that matter most

1. **A permission rule that did not fire.** Use the *gate widening* issue template. Every defect of
   this kind found so far was silent, so an exact call and an exact rule is worth more than a
   feature request.
2. **A vendor release that moved something.** Claude Code, Copilot and Codex change weekly. Use the
   *vendor drift* template and quote the changelog row.

## Before a pull request

```sh
just check          # fmt, clippy, build, test — what CI runs
just verify         # plus the claim ledger and the dependency count; needs `just specs` once
just perms 20       # a quick differential pass against a real `claude`, if you have one
```

Every behaviour change to the permission layer needs a test that asserts **both** the verdict and
the rule that produced it; a right answer with a wrong reason is a bug here. A dependency bump is a
reviewed change: agents and packages are pinned on purpose.

The `ui/` directory is one HTML file with no build step. Keep it that way; if a change needs a
bundler, open an issue first. `just ui` serves it from disk, so editing it is a browser reload.
`tests/ui_contract.rs` holds it to the API, to escaping every value it prints, and to rendering at
all — that last one needs `node`, and skips without it.

## Scope

Things that will not be merged, so nobody spends a weekend on them: a model deciding a permission
or a gate result; a cloud relay or account; a second rule language; parsing transcript JSONL on the
critical path; a React or WASM rewrite of the board. The public docs explain the reasoning:
<https://hupe1980.github.io/vibeplane/docs/decisions/>.

## Licence

MIT OR Apache-2.0, at your option. By contributing you agree your contribution is licensed the same way.

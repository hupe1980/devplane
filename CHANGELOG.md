# Changelog

Notable changes per release. Dates are UTC.

## 0.2.0 — 2026-09-15

Several permission rules now decide differently, after checking them against a
running Claude Code. Read **Changed** before upgrading: two of them can make an
existing `never_auto` rule cover less than it did.

### Added

- `vibeplane explain --replay` — replays every tool call already observed against
  the current rules, and names the rule that would answer the ones that reached
  you. Offline; `--dir` scopes it to one project.
- `vibeplane doctor` names the model provider, and on Bedrock, Google Cloud's
  Agent Platform, Microsoft Foundry, a Console key or a gateway says which of
  Claude Code's own supervision surfaces are unavailable there.
- Permission items in `vibeplane inbox` name the rule that would have answered
  them.
- A pipeline `review` step falls back to the repository's `REVIEW.md`.
- `vibeplane check` notes when a `Read(path)` deny has no `Edit(path)` beside it.
- `just rows` checks every rule-relevant row of Claude Code's changelog against
  a committed ledger; `just perms-allow` and `just perms-deny` run one half of
  the differential harness.

### Changed

- **`Read(path)` deny no longer covers a shell redirect or `touch`.** It still
  covers a recognised file command that writes, such as `tee`. To protect a file
  from a shell, write both `Read(path)` and `Edit(path)`.
- **`mv` operands are writes.** `mv` removes its source, so an `Edit` deny stops
  it. `cp` is unchanged.
- Deny rules reach further: option values (`grep -f.env`), `git diff`/`git grep`
  operands, everything under a directory a `grep -r` or `cp -r` walks, and
  whatever `env` or `sudo` runs.
- Many more reader commands are covered — `awk`, `sort`, `od`, `strings`, `jq`,
  `base64`, `wc`, `diff` and others. `xxd`, `zcat`, `join`, `less`, `more` and
  `truncate` are **not**: Claude Code does not recognise them either.
- Symlinks resolve from both ends, so a rule naming `/tmp` also covers
  `/private/tmp`.
- A leading assignment that runs something — `DIRSTACKSIZE=$(id) ls`,
  `OPTIND=1/0 ls` — is no longer treated as a read-only command.
- No allow rule approves a command behind `env`, `eval`, `sudo`, `doas` or
  `exec`. Deny rules see through them, which is stricter than Claude Code and
  deliberate.
- `vibeplane ls` says why the cost and context columns are blank when nothing is
  connected.

### Fixed

- **`vibeplane open` served a board that never loaded.** Two `const hit` in one
  block scope is a `SyntaxError`, so the whole script failed to parse: the page
  rendered its chrome, said "connecting", and fetched nothing. The board's
  script is now parsed by the test suite.
- A multi-byte character in an option value crashed the permission matcher, and
  with it every tool call waiting on the hook.
- `sed -n 1p .env` and `grep -f pats.txt .env` named no file, so a deny rule on
  it did nothing.
- An allow rule naming an exact compound command approved nothing.
- `Bash(rule) trailing text` is reported as text after the closing bracket rather
  than as a missing one.

## 0.1.0 — 2026-09-14

First release. Watching Claude Code sessions, driving any Agent Client Protocol
agent, verified-done with project gates, declared pipelines, a per-repository
permission gate, and the decision log.

# Vibeplane

The local-first control plane for AI coding agents — Claude Code first, Codex and OpenCode next.

- One board across all projects and providers, including sessions you started in VS Code or a terminal
- An attention inbox that shows only what needs a human, answerable from the keyboard
- Work items with verification gates: *verified done*, not *agent says done*
- Git worktrees, GitHub issues/PRs/releases, dependency-aware batches
- Rust core (daemon + CLI), React UI in a Tauri shell, no cloud account

Status: concept stage. Read [CONCEPT.md](CONCEPT.md). The `vibeplane` crate and npm package are placeholder releases that reserve the name.

## Layout

- `crates/vibeplane` — CLI crate (placeholder today)
- `npm/vibeplane` — npm placeholder
- `scripts/fetch-specs.sh` — downloads the third-party specs referenced by the concept into `specs/` (gitignored)

## License

MIT OR Apache-2.0

# Changelog

## [0.12.0] - unreleased

### Added

- `speckit.devplane.gate`: runs the project's `[gates]` with `devplane gate --json` and reports the
  verdict verbatim.
- A mandatory `after_implement` hook for it, with no `condition`.
- `overlays/devplane-gate.yml`: a workflow overlay that runs the gate as a `shell` step after
  `implement` in the `speckit` workflow.

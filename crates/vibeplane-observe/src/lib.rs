//! Observation channels.
//!
//! Everything Vibeplane learns about a session it did not start arrives through
//! one of these, and every one of them is a documented interface:
//!
//! * [`hook`] — Claude Code hooks: lifecycle, blocking, and the policy gate.
//! * [`otel`] — OpenTelemetry export: per-request cost, tokens and entrypoint.
//! * [`agents_json`] — the background roster, authoritative for what the
//!   provider's own daemon supervises.
//! * [`statusline`] — the optional shim, for rate limits.
//! * [`connect`] — installing and removing the above, carefully.
//! * [`locate`] — finding the `claude` binary, which is routinely not on `PATH`.
//! * [`registry`] — the local session and editor-window files. Undocumented
//!   internals, so strictly enrichment: they add the entrypoint and the window
//!   to focus, and their absence costs nothing else.
//!
//! The transcript files under `~/.claude/projects` are deliberately absent.
//! Their format is documented as internal and changing between releases, so
//! parsing them would make every Claude Code update a coin toss.

pub mod agents_json;
pub mod connect;
pub mod hook;
pub mod locate;
pub mod otel;
pub mod registry;
pub mod statusline;

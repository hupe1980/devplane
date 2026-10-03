//! Devplane — records who decided, when nobody asked you, across every project
//! and every coding agent on your machine.
//!
//! [`core`] is the pure half — types, reducer, attention, permission policy —
//! and `tests/purity.rs` fails the build if it reaches the outside world. The
//! binary is a thin parser over this library so tests can drive every seam.

/// The Agent Client Protocol client: driving any agent that speaks it.
pub mod acp;
/// The HTTP surface: receivers for the providers, an API for the clients.
pub mod api;
/// The window: the host with a tray, notifications and a shortcut (`app` feature).
#[cfg(feature = "app")]
pub mod app;
/// The base's tests on a change and its new tests on the base, in one
/// throwaway checkout at the merge base.
pub mod base_checks;
/// The loop that makes a run mean something.
pub(crate) mod change;
/// The command line, one module per thing a person is trying to do.
pub mod cli;
/// The CLI's view of the host.
pub(crate) mod client;
/// A person's review comments, kept until sent as one prompt.
pub(crate) mod comments;
/// Where Devplane keeps its own state, and how a client finds the host.
pub mod config;
/// Types and the pure logic over them. Reaches nothing outside the process.
pub mod core;
/// Runs Devplane owns, over the Agent Client Protocol.
pub mod draft;
pub mod driven;
/// The first run from the window: its checklist and the writes it confirms.
pub mod firstrun;
/// Verification gates: the project's own definition of done, run and parsed.
pub mod gates;
/// Git: worktrees per unit of work, and the status the board shows.
pub mod git;
/// GitHub, spoken to directly: sign-in, issues, pull requests, checks.
pub mod github;
/// The one answer the window gave and has not sent yet, and its undo.
pub mod hold;
/// `devplane hook` and `devplane statusline`, run in the vendor's process.
pub mod hook;
/// The host: receivers, API and the authoritative state.
pub mod host;
/// Reading the store with nothing running, into the same views the host serves.
pub mod local;
/// An MCP server agents can ask and cannot act through.
pub(crate) mod mcp;
/// Desktop notifications.
pub mod notify;
/// Observation channels: hooks, OpenTelemetry, the roster, the status line.
pub mod observe;
/// The policy that governs a directory, read from disk and cached.
pub mod policy_cache;
/// Background loops: the roster poller, the stall sweeper, the PR watcher.
pub mod poller;
/// What a short-lived process writes down, with no host in the path.
pub mod record;
/// Terminal output.
pub mod render;
/// Where a directory belongs on disk: repository, worktree owner, governing
/// root.
pub mod repo;
/// What a repository already has set up for agents, read from disk.
pub(crate) mod setup;
/// The person's own SSH signature on an attested statement, at export.
pub mod sign;
/// The specification a change answers, read back from disk.
pub mod spec;
/// The wall clock, read at the edge: the pure half's constructors stamped
/// with now.
pub(crate) mod stamp;
/// The observation store: SQLite, WAL, rebuildable.
pub mod store;
/// Which repositories a person trusted: a file of its own, never the store.
pub mod trust;
/// Whether a newer version exists, as the window's update check found it.
/// No network here: the check is the app's, and only once the person said so.
pub mod update;
/// `devplane verify`: the verdict where merges are decided, with no host.
pub mod verify;
/// What the surfaces show, composed once for every caller.
pub mod view;

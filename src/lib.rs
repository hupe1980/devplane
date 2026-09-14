//! Vibeplane — the local-first control plane for AI coding agents.
//!
//! One crate. [`core`] is the half that may not reach the outside world — the
//! types, the reducer, the attention engine, the permission policy — and
//! `tests/purity.rs` fails the build if it ever does. Everything else here
//! touches something: the daemon, the receivers that learn what other people's
//! agents are doing, the client that drives agents of our own, and the layers
//! that turn "the agent says it is done" into "the project's own checks agree".
//!
//! The binary is a thin argument parser over this library. Everything
//! interesting in this product happens at the seam with a provider, and a seam
//! that can only be exercised through a subprocess is a seam nobody exercises.

/// The Agent Client Protocol client: driving any agent that speaks it.
pub mod acp;
/// The HTTP surface: receivers for the providers, an API for the clients.
pub mod api;
/// The command line, one module per thing a person is trying to do.
pub mod cli;
/// The CLI's view of the daemon.
pub mod client;
/// Where Vibeplane keeps its own state, and how a client finds the daemon.
pub mod config;
/// Types and the pure logic over them. Reaches nothing outside the process.
pub mod core;
/// The daemon: receivers, API and the authoritative state.
pub mod daemon;
/// Starting the daemon in the background.
pub mod daemonise;
/// Runs Vibeplane owns, over the Agent Client Protocol.
pub mod driven;
/// Raising the window that owns a session.
pub mod focus;
/// Verification gates: the project's own definition of done, run and parsed.
pub mod gates;
/// Git: worktrees per unit of work, and the status the board shows.
pub mod git;
/// GitHub, through the `gh` command: issues, pull requests, checks.
pub mod github;
/// Desktop notifications.
pub mod notify;
/// Observation channels: hooks, OpenTelemetry, the roster, the status line.
pub mod observe;
/// Declared chains of agent runs.
pub mod pipeline;
/// Background loops: the roster poller, the stall sweeper, the PR watcher.
pub mod poller;
/// Terminal output.
pub mod render;
/// The observation store: SQLite, WAL, rebuildable.
pub mod store;
/// The loop that makes a run mean something.
pub mod work;

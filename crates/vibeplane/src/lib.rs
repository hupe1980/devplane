//! Vibeplane — the local-first control plane for AI coding agents.
//!
//! The daemon, the receivers and the client live here so that the binary is a
//! thin argument parser over a library the tests can drive directly. Everything
//! interesting in this product happens at the seam with a provider, and a seam
//! that can only be exercised through a subprocess is a seam nobody exercises.

pub mod api;
pub mod client;
pub mod config;
pub mod daemon;
pub mod daemonise;
pub mod driven;
pub mod focus;
pub mod notify;
pub mod poller;
pub mod render;

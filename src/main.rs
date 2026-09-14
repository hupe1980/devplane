//! Vibeplane — the local-first control plane for AI coding agents.
//!
//! One binary. `vibeplane serve` is the daemon that receives hooks and
//! telemetry; every other subcommand is a client of it and starts it if it is
//! not already running, so the observer is never something the user has to
//! remember.
//!
//! This file is the argument parser and nothing else. The commands live in
//! `vibeplane::cli`, in the library, so the tests can drive them directly.

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    vibeplane::cli::run(vibeplane::cli::Cli::parse()).await
}

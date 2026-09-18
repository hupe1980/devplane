//! Devplane — the local-first control plane for AI coding agents.
//!
//! One binary. `devplane serve` is the daemon that receives hooks and
//! telemetry; every other subcommand is a client of it and starts it if it is
//! not already running, so the observer is never something the user has to
//! remember.
//!
//! This file is the argument parser and nothing else. The commands live in
//! `devplane::cli`, in the library, so the tests can drive them directly.

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    devplane::cli::run(devplane::cli::Cli::parse()).await
}

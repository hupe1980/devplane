//! Devplane — records who decided, when nobody asked you, across every project
//! and every coding agent on your machine.
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
    // **The completion helper is answered before clap sees the arguments, and
    // that is the whole reason it is here.**
    //
    // It was a `#[command(hide = true)]` subcommand, which hides it from the
    // help screen and **not from `clap_complete`**: the generated zsh script
    // carried `'complete:The live values behind a completion. Not for people'`
    // as an offered command, so the one thing that must never appear in a
    // completion list was in every completion list.
    //
    // A command clap does not know about cannot leak into anything clap
    // generates. Stripping it out of the generated script afterwards would be
    // the same fix with a text filter holding it up.
    let mut args = std::env::args_os();
    if args.len() == 3
        && args
            .nth(1)
            .is_some_and(|a| a == devplane::cli::COMPLETE_ARG)
        && let Some(what) = args.next()
    {
        return devplane::cli::completions::cmd_complete(&what.to_string_lossy()).await;
    }
    devplane::cli::run(devplane::cli::Cli::parse()).await
}

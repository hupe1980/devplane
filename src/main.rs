//! Devplane — records who decided, when nobody asked you, across every project
//! and every coding agent on your machine.
//!
//! This file is the argument parser; the commands live in `devplane::cli` so
//! tests can drive them directly.

use clap::Parser;

#[tokio::main]
async fn main() {
    // The completion helper is answered before clap sees the arguments: a
    // command clap does not know about cannot leak into the completion scripts
    // `clap_complete` generates (a hidden subcommand still would).
    let mut args = std::env::args_os();
    if args.len() == 3
        && args
            .nth(1)
            .is_some_and(|a| a == devplane::cli::COMPLETE_ARG)
        && let Some(what) = args.next()
    {
        if let Err(e) = devplane::cli::completions::cmd_complete(&what.to_string_lossy()).await {
            std::process::exit(devplane::cli::report_error(&e, false));
        }
        return;
    }
    let cli = devplane::cli::Cli::parse();
    // Read before the command consumes the arguments: an error under `--json`
    // is printed as JSON.
    let json = cli.json();
    if let Err(e) = devplane::cli::run(cli).await {
        std::process::exit(devplane::cli::report_error(&e, json));
    }
}

//! `wubbie` command-line entry point.
//!
//! Thin entrypoint: parse the args, initialize logging, and delegate to the
//! selected subcommand's handler. The CLI layout (entrypoint here, argument
//! structs under `config/`, handlers under `cmd/`) follows the house
//! convention; see MULTI-1382.

use anyhow::Result;
use clap::{CommandFactory, Parser};
use tracing::level_filters::LevelFilter;

use wubbie::Cli;

fn main() -> Result<()> {
    // Parse the args provided to this process, including commands and flags.
    let cli = Cli::parse();
    // Execute whichever command was requested.
    dispatch_command(cli)
}

/// Initialize logging from the global flags, then delegate to the requested
/// command's handler.
fn dispatch_command(cli: Cli) -> Result<()> {
    init_tracing(*cli.log_level());
    match cli.cmd() {
        Some(cmd) => cmd.clone().dispatch(),
        // No command was provided.
        None => empty_command(),
    }
}

/// When the CLI is run without any command, print the long help and exit
/// successfully.
fn empty_command() -> Result<()> {
    Cli::command()
        .print_long_help()
        .expect("unable to print help message");
    Ok(())
}

/// Install the tracing subscriber at the requested maximum level.
fn init_tracing(level: LevelFilter) {
    tracing_subscriber::fmt().with_max_level(level).init();
}

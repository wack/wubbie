//! `wubbie` command-line entry point.
//!
//! This is a thin CLI scaffold. The subcommands are wired up but not yet
//! implemented; the layout convention they follow is tracked in MULTI-1382.

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "wubbie",
    version,
    about = "A fully-open SLM, from corpus to inference"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Train the model from a corpus.
    Train,
    /// Generate text from a trained model.
    Generate,
    /// Serve the model for inference.
    Serve,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Train => anyhow::bail!("`wubbie train` is not implemented yet"),
        Command::Generate => anyhow::bail!("`wubbie generate` is not implemented yet"),
        Command::Serve => anyhow::bail!("`wubbie serve` is not implemented yet"),
    }
}

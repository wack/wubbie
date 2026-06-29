//! The top-level subcommand enum and its dispatch.

use anyhow::Result;
use clap::Subcommand;

use super::{GenerateSubcommand, ServeSubcommand, TrainSubcommand};
use crate::cmd::{Generate, Serve, Train};

/// A `Command` is one of the top-level commands accepted by the `wubbie` CLI.
///
/// Each variant carries its parsed argument struct (defined under
/// [`crate::config`]) and dispatches to the matching handler under
/// [`crate::cmd`]. Subcommands added in later tickets inherit this same
/// structure: an `Args` struct in `config/` and a handler in `cmd/`.
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Train the model from a corpus.
    Train(TrainSubcommand),
    /// Generate text from a trained model.
    Generate(GenerateSubcommand),
    /// Serve the model for inference.
    Serve(ServeSubcommand),
}

impl Command {
    /// Dispatch the parsed arguments to the matching command handler.
    pub fn dispatch(self) -> Result<()> {
        match self {
            Self::Train(args) => Train::new(args)?.dispatch(),
            Self::Generate(args) => Generate::new(args)?.dispatch(),
            Self::Serve(args) => Serve::new(args)?.dispatch(),
        }
    }
}

//! The top-level subcommand enum and its dispatch.

use anyhow::Result;
use clap::Subcommand;

use super::{
    DownloadSubcommand, GenerateSubcommand, ServeSubcommand, TokenizerSubcommand, TrainSubcommand,
};
use crate::cmd::{Download, Generate, Serve, Tokenizer, Train};

/// A `Command` is one of the top-level commands accepted by the `wubbie` CLI.
///
/// Each variant carries its parsed argument struct (defined under
/// [`crate::config`]) and dispatches to the matching handler under
/// [`crate::cmd`]. Subcommands added in later tickets inherit this same
/// structure: an `Args` struct in `config/` and a handler in `cmd/`.
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Download the corpus from Hugging Face into the local cache.
    Download(DownloadSubcommand),
    /// Train the byte-level BPE tokenizer on a corpus slice.
    Tokenizer(TokenizerSubcommand),
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
            Self::Download(args) => Download::new(args)?.dispatch(),
            Self::Tokenizer(args) => Tokenizer::new(args)?.dispatch(),
            Self::Train(args) => Train::new(args)?.dispatch(),
            Self::Generate(args) => Generate::new(args)?.dispatch(),
            Self::Serve(args) => Serve::new(args)?.dispatch(),
        }
    }
}

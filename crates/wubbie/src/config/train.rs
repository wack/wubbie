//! Arguments for `wubbie train`.

use std::path::{Path, PathBuf};

use clap::Args;

use super::ModelSize;

/// `wubbie train`: train the model from a corpus.
///
/// Scaffold arguments — the training loop is implemented in a later ticket.
/// `--config` selects a serialized [`RunConfig`](super::RunConfig) for a
/// reproducible run; `--size` picks a named architecture when no config file is
/// given.
#[derive(Debug, Args, Clone)]
pub struct TrainSubcommand {
    /// Path to a serialized run config (overrides `--size`).
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Named model size to train when no `--config` is supplied.
    #[arg(long, value_enum, default_value_t = ModelSize::Gpt2Small)]
    size: ModelSize,
}

impl TrainSubcommand {
    /// Path to the run-config file, if one was supplied.
    pub fn config(&self) -> Option<&Path> {
        self.config.as_deref()
    }

    /// The named model size to train.
    pub fn size(&self) -> ModelSize {
        self.size
    }
}

//! Arguments for `wubbie train`.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;

use super::{ModelConfig, ModelConfigArgs, ModelSize, load_model_config};

/// `wubbie train`: train the model from a corpus.
///
/// Scaffold arguments — the training loop is implemented in a later ticket.
/// `--size` picks the named architecture base; `--config` layers a (possibly
/// partial) config file on top of it; individual `--d-model`/`--num-layers`/…
/// flags are the highest-precedence overrides. See
/// [`resolve_model_config`](Self::resolve_model_config).
#[derive(Debug, Args, Clone)]
pub struct TrainSubcommand {
    /// Path to a config file layered over `--size` (`.toml` or `.json`).
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Named model size used as the base layer.
    #[arg(long, value_enum, default_value_t = ModelSize::Gpt2Small)]
    size: ModelSize,

    /// Per-field model-dimension overrides (highest precedence).
    #[command(flatten)]
    model: ModelConfigArgs,
}

impl TrainSubcommand {
    /// Path to the config file, if one was supplied.
    pub fn config(&self) -> Option<&Path> {
        self.config.as_deref()
    }

    /// The named model size used as the base layer.
    pub fn size(&self) -> ModelSize {
        self.size
    }

    /// Resolve the fully-specified [`ModelConfig`] by merging, in increasing
    /// precedence: the `--size` base, the `--config` file, the `WUBBIE_MODEL_`
    /// environment layer, and the per-field override flags.
    pub fn resolve_model_config(&self) -> Result<ModelConfig> {
        load_model_config(&self.size.config(), self.config.as_deref(), &self.model)
    }
}

//! The `wubbie train` command handler.

use anyhow::Result;

use crate::config::{TrainSubcommand, read_model_env_overrides};

/// Handler for `wubbie train`.
pub struct Train {
    args: TrainSubcommand,
}

impl Train {
    pub fn new(args: TrainSubcommand) -> Result<Self> {
        Ok(Self { args })
    }

    pub fn dispatch(self) -> Result<()> {
        // Resolve the layered config now so configuration errors surface before
        // the (not-yet-implemented) training loop would run. The env layer is
        // captured from `std::env` here at the CLI boundary; the loader itself
        // stays pure so tests can drive it without process-env races.
        let env = read_model_env_overrides();
        let model = self.args.resolve_model_config(&env)?;
        tracing::info!(?model, "resolved model config");
        anyhow::bail!("`wubbie train` is not implemented yet")
    }
}

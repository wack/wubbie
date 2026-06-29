//! The `wubbie train` command handler.

use anyhow::Result;

use crate::config::TrainSubcommand;

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
        // the (not-yet-implemented) training loop would run.
        let model = self.args.resolve_model_config()?;
        tracing::info!(?model, "resolved model config");
        anyhow::bail!("`wubbie train` is not implemented yet")
    }
}

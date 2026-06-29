//! The `wubbie train` command handler.

use anyhow::Result;

use crate::config::TrainSubcommand;

/// Handler for `wubbie train`.
pub struct Train {
    #[allow(dead_code)] // consumed once the training loop lands.
    args: TrainSubcommand,
}

impl Train {
    pub fn new(args: TrainSubcommand) -> Result<Self> {
        Ok(Self { args })
    }

    pub fn dispatch(self) -> Result<()> {
        anyhow::bail!("`wubbie train` is not implemented yet")
    }
}

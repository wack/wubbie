//! The `wubbie serve` command handler.

use anyhow::Result;

use crate::config::ServeSubcommand;

/// Handler for `wubbie serve`.
pub struct Serve {
    #[allow(dead_code)] // consumed once the inference server lands.
    args: ServeSubcommand,
}

impl Serve {
    pub fn new(args: ServeSubcommand) -> Result<Self> {
        Ok(Self { args })
    }

    pub fn dispatch(self) -> Result<()> {
        anyhow::bail!("`wubbie serve` is not implemented yet")
    }
}

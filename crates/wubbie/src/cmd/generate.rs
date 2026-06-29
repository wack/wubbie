//! The `wubbie generate` command handler.

use anyhow::Result;

use crate::config::GenerateSubcommand;

/// Handler for `wubbie generate`.
pub struct Generate {
    #[allow(dead_code)] // consumed once sampling lands (T3).
    args: GenerateSubcommand,
}

impl Generate {
    pub fn new(args: GenerateSubcommand) -> Result<Self> {
        Ok(Self { args })
    }

    pub fn dispatch(self) -> Result<()> {
        anyhow::bail!("`wubbie generate` is not implemented yet")
    }
}

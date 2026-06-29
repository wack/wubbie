//! Arguments for `wubbie generate`.

use std::path::{Path, PathBuf};

use clap::Args;

/// `wubbie generate`: generate text from a trained model.
///
/// Scaffold arguments — sampling is implemented in a later ticket (T3), which
/// inherits this layout. `--weights` points at a safetensors checkpoint;
/// `--prompt` is the conditioning text.
#[derive(Debug, Args, Clone)]
pub struct GenerateSubcommand {
    /// Path to the safetensors weights to load.
    #[arg(long, value_name = "FILE")]
    weights: Option<PathBuf>,

    /// The prompt to condition generation on.
    #[arg(long)]
    prompt: Option<String>,
}

impl GenerateSubcommand {
    /// Path to the weights file, if one was supplied.
    pub fn weights(&self) -> Option<&Path> {
        self.weights.as_deref()
    }

    /// The conditioning prompt, if one was supplied.
    pub fn prompt(&self) -> Option<&str> {
        self.prompt.as_deref()
    }
}

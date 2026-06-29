//! Arguments for `wubbie serve`.

use std::path::{Path, PathBuf};

use clap::Args;

/// `wubbie serve`: serve the model for inference.
///
/// Scaffold arguments — the inference server is implemented in a later ticket.
/// `--weights` points at a safetensors checkpoint; `--port` is the bind port.
#[derive(Debug, Args, Clone)]
pub struct ServeSubcommand {
    /// Path to the safetensors weights to load.
    #[arg(long, value_name = "FILE")]
    weights: Option<PathBuf>,

    /// TCP port to bind the inference server to.
    #[arg(long, default_value_t = 8080)]
    port: u16,
}

impl ServeSubcommand {
    /// Path to the weights file, if one was supplied.
    pub fn weights(&self) -> Option<&Path> {
        self.weights.as_deref()
    }

    /// The port to bind to.
    pub fn port(&self) -> u16 {
        self.port
    }
}

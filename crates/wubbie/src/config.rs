//! Model configuration types.
//!
//! This is a baseline, serde-serializable description of the model
//! architecture. The full configuration system (and the CLI layout convention
//! that loads it) is tracked in MULTI-1382.

use serde::{Deserialize, Serialize};

/// Hyperparameters describing the model architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Number of tokens in the tokenizer vocabulary.
    pub vocab_size: usize,
    /// Maximum sequence length the model attends over.
    pub context_length: usize,
    /// Hidden dimension of the residual stream.
    pub d_model: usize,
    /// Number of transformer blocks.
    pub num_layers: usize,
    /// Number of attention heads per block.
    pub num_heads: usize,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            vocab_size: 32_000,
            context_length: 1_024,
            d_model: 512,
            num_layers: 8,
            num_heads: 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let config = ModelConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: ModelConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }
}

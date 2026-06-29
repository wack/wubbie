//! Reproducible-run configuration: architecture plus optimization, the bundle
//! the training loop instantiates against and that a run is reproduced from.

use serde::{Deserialize, Serialize};

use super::ModelConfig;

/// Optimization hyperparameters for the training loop.
///
/// Phase 3 owns the final values; the [`Default`] here is a reasonable
/// provisional starting point so the substrate is in place and the training
/// loop has a struct to instantiate against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Peak learning rate.
    pub learning_rate: f64,
    /// AdamW weight-decay coefficient.
    pub weight_decay: f64,
    /// Examples per optimization step.
    pub batch_size: usize,
    /// Total number of optimization steps.
    pub max_steps: usize,
    /// Linear warmup steps before the learning-rate schedule decays.
    pub warmup_steps: usize,
    /// Optional global gradient-norm clipping threshold.
    pub grad_clip: Option<f64>,
    /// RNG seed, recorded so a run is reproducible.
    pub seed: u64,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            learning_rate: 3e-4,
            weight_decay: 0.1,
            batch_size: 32,
            max_steps: 100_000,
            warmup_steps: 2_000,
            grad_clip: Some(1.0),
            seed: 0,
        }
    }
}

/// A complete, reproducible run description: architecture plus optimization.
///
/// Serializing a `RunConfig` captures everything needed to reproduce a run; the
/// training loop and inference paths both instantiate against it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunConfig {
    /// Model architecture.
    pub model: ModelConfig,
    /// Optimization hyperparameters.
    pub training: TrainingConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_config_round_trips_through_json() {
        let config = RunConfig::default();
        let json = serde_json::to_string_pretty(&config).expect("serialize");
        let parsed: RunConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }
}

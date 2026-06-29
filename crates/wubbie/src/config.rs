//! Model and run configuration — the single source of truth the model, the
//! loss, and the training loop all instantiate against.
//!
//! The types here are `serde`-serializable so a run can be described entirely by
//! a config and reproduced from it. The *on-disk* configuration-file layout
//! (and the CLI that loads it) follows the house convention tracked in
//! MULTI-1382 and is intentionally not pinned here — these structs are
//! format-agnostic and round-trip through any `serde` data format.

use serde::{Deserialize, Serialize};

use crate::tokenizer::VOCAB_SIZE;

/// The context length (maximum sequence length) shared by every named config.
pub const CONTEXT_LENGTH: usize = 1_024;

/// Architecture hyperparameters describing the decoder-only transformer.
///
/// Every field is an integer or a bool so the struct derives [`Eq`] and hashes
/// cleanly; the training-time floating-point hyperparameters live separately in
/// [`TrainingConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Number of tokens in the tokenizer vocabulary. Wired to [`VOCAB_SIZE`].
    pub vocab_size: usize,
    /// Maximum sequence length the model attends over (`context_len`).
    pub context_length: usize,
    /// Hidden dimension of the residual stream (`d_model`).
    pub d_model: usize,
    /// Inner dimension of the position-wise feed-forward sublayer (`d_ff`).
    pub d_ff: usize,
    /// Number of stacked transformer blocks (`n_layers`).
    pub num_layers: usize,
    /// Number of attention heads per block (`n_heads`).
    pub num_heads: usize,
    /// Layer-norm placement toggle.
    ///
    /// `false` = **post-norm**, the original "Attention Is All You Need"
    /// arrangement (norm *after* each sublayer) — paper-faithful.
    /// `true` = **pre-norm** (norm *before* each sublayer) — the modern,
    /// training-stable variant used by GPT-2 and most contemporary
    /// decoder-only models.
    ///
    /// The project-wide default is a Phase 3 decision (MULTI-1382 defers it);
    /// the named configs below ship a *provisional* pre-norm setting so the
    /// surface exists and is wired through. It is not a final architectural
    /// commitment.
    pub norm_first: bool,
}

impl ModelConfig {
    /// The headline ~100M-parameter, GPT-2-small-class configuration.
    ///
    /// 12 layers × 768-wide × 12 heads with a 4× (3072) feed-forward dimension.
    /// With the placeholder tokenizer vocabulary this lands at roughly 110M
    /// parameters (see [`ModelConfig::approx_parameter_count`]).
    pub fn gpt2_small() -> Self {
        Self {
            vocab_size: VOCAB_SIZE,
            context_length: CONTEXT_LENGTH,
            d_model: 768,
            d_ff: 3_072,
            num_layers: 12,
            num_heads: 12,
            norm_first: true,
        }
    }

    /// A deliberately tiny configuration for fast tests and local smoke runs.
    pub fn debug_tiny() -> Self {
        Self {
            vocab_size: VOCAB_SIZE,
            context_length: CONTEXT_LENGTH,
            d_model: 128,
            d_ff: 512,
            num_layers: 2,
            num_heads: 2,
            norm_first: true,
        }
    }

    /// The per-head dimension, i.e. `d_model / num_heads`.
    ///
    /// Returns `None` when `d_model` is not divisible by `num_heads`, which is a
    /// misconfiguration callers should reject.
    pub fn head_dim(&self) -> Option<usize> {
        // `is_multiple_of(0)` is false for a nonzero `d_model`, so this also
        // rejects `num_heads == 0` without a separate guard.
        self.d_model
            .is_multiple_of(self.num_heads)
            .then(|| self.d_model / self.num_heads)
    }

    /// Approximate total parameter count.
    ///
    /// Assumes learned token + positional embeddings, a tied (weight-shared)
    /// language-model head, and biased linear layers. This is a sizing estimate
    /// for picking a named config, not an exact count for a specific model
    /// implementation.
    pub fn approx_parameter_count(&self) -> usize {
        let embeddings = (self.vocab_size + self.context_length) * self.d_model;
        // Per block: QKV + output projections, two layer norms, and the FFN.
        let attention = 4 * self.d_model * self.d_model + 4 * self.d_model;
        let layer_norms = 4 * self.d_model;
        let feed_forward = 2 * self.d_model * self.d_ff + self.d_ff + self.d_model;
        let per_block = attention + layer_norms + feed_forward;
        let final_norm = 2 * self.d_model;
        embeddings + self.num_layers * per_block + final_norm
    }
}

impl Default for ModelConfig {
    /// The headline GPT-2-small-class config is the default.
    fn default() -> Self {
        Self::gpt2_small()
    }
}

/// A named, predefined model size.
///
/// Later tickets surface this on the CLI (subject to the layout sign-off in
/// MULTI-1382); it exists here so the named sizes have a single, serializable
/// enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelSize {
    /// The ~100M-parameter, GPT-2-small-class target.
    Gpt2Small,
    /// A tiny size for fast tests and local smoke runs.
    DebugTiny,
}

impl ModelSize {
    /// Materialize the [`ModelConfig`] for this named size.
    pub fn config(self) -> ModelConfig {
        match self {
            ModelSize::Gpt2Small => ModelConfig::gpt2_small(),
            ModelSize::DebugTiny => ModelConfig::debug_tiny(),
        }
    }
}

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
    fn gpt2_small_instantiates_in_the_100m_class() {
        let config = ModelConfig::gpt2_small();
        let params = config.approx_parameter_count();
        // The headline target is "~100M, GPT-2-small-class"; assert it lands in
        // that ballpark rather than pinning an exact, implementation-specific
        // count.
        assert!(
            (80_000_000..=140_000_000).contains(&params),
            "expected ~100M parameters, got {params}",
        );
    }

    #[test]
    fn vocab_size_is_wired_to_the_tokenizer() {
        assert_eq!(ModelConfig::gpt2_small().vocab_size, VOCAB_SIZE);
        assert_eq!(ModelConfig::debug_tiny().vocab_size, VOCAB_SIZE);
    }

    #[test]
    fn context_length_is_1024() {
        assert_eq!(ModelConfig::gpt2_small().context_length, 1_024);
    }

    #[test]
    fn norm_first_toggle_is_present_and_round_trips() {
        let mut config = ModelConfig::gpt2_small();
        config.norm_first = false;
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: ModelConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(!parsed.norm_first);
    }

    #[test]
    fn head_dim_divides_cleanly_for_named_configs() {
        assert_eq!(ModelConfig::gpt2_small().head_dim(), Some(64));
        assert_eq!(ModelConfig::debug_tiny().head_dim(), Some(64));
    }

    #[test]
    fn head_dim_rejects_indivisible_shapes() {
        let config = ModelConfig {
            num_heads: 7,
            ..ModelConfig::gpt2_small()
        };
        assert_eq!(config.head_dim(), None);
    }

    #[test]
    fn named_size_round_trips_to_its_config() {
        assert_eq!(ModelSize::Gpt2Small.config(), ModelConfig::gpt2_small());
        assert_eq!(ModelSize::DebugTiny.config(), ModelConfig::debug_tiny());
    }

    #[test]
    fn run_config_round_trips_through_json() {
        let config = RunConfig::default();
        let json = serde_json::to_string_pretty(&config).expect("serialize");
        let parsed: RunConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }

    #[test]
    fn model_size_serializes_kebab_case() {
        let json = serde_json::to_string(&ModelSize::Gpt2Small).expect("serialize");
        assert_eq!(json, "\"gpt2-small\"");
    }
}

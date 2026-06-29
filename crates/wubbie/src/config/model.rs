//! Model architecture configuration — the dims the model and loss instantiate
//! against.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::tokenizer::VOCAB_SIZE;

/// The context length (maximum sequence length) shared by every named config.
pub const CONTEXT_LENGTH: usize = 1_024;

/// Architecture hyperparameters describing the decoder-only transformer.
///
/// Every field is an integer or a bool so the struct derives [`Eq`] and hashes
/// cleanly; the training-time floating-point hyperparameters live separately in
/// [`TrainingConfig`](super::TrainingConfig).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Number of tokens in the tokenizer vocabulary.
    ///
    /// Seeded from the [`VOCAB_SIZE`] default, but the **trained tokenizer is the
    /// authority**: when `wubbie train --tokenizer <file>` is given, this is
    /// overridden with the tokenizer's actual size (see
    /// [`TrainSubcommand::resolve_model_config`](super::TrainSubcommand) and
    /// [`tokenizer::vocab_size_from_file`](crate::tokenizer::vocab_size_from_file)).
    /// The embedding table and LM head must match the tokenizer exactly, so the
    /// model is sized from this resolved value, never from the constant directly.
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

/// A named, predefined model size, selectable on the CLI (`--size`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
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

/// A partial [`ModelConfig`]: every field optional so a config file (or a CLI
/// override layer) may specify only some of them.
///
/// This is the shape each non-default layer deserializes into during the merge
/// in [`load_model_config`](super::load_model_config). Only the fields that are
/// actually set serialize (`skip_serializing_if`), so an unset value in a
/// higher-precedence layer contributes nothing and never clobbers a lower one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialModelConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vocab_size: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d_model: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d_ff: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_layers: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_heads: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub norm_first: Option<bool>,
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
    fn model_size_serializes_kebab_case() {
        let json = serde_json::to_string(&ModelSize::Gpt2Small).expect("serialize");
        assert_eq!(json, "\"gpt2-small\"");
    }
}

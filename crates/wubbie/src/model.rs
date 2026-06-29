//! Decoder-only transformer: model definition, forward pass, and next-token
//! cross-entropy loss (MULTI-1383).
//!
//! [`DecoderModel`] is the assembled GPT-style language model — token
//! embeddings, sinusoidal absolute positional encoding, a stack of
//! pre/post-norm transformer blocks with a **causal** attention mask, a final
//! [`LayerNorm`], and an untied LM head. The block stack is Burn's
//! [`TransformerEncoder`] driven by the `mask_attn` argument: a decoder-only GPT
//! has no cross-attention or encoder memory, so `TransformerDecoder` would
//! silently wire in cross-attention to memory that doesn't exist. The encoder +
//! a causal mask is the correct primitive.
//!
//! [`DecoderModel::forward`] takes a `[batch × seq]` token tensor and returns
//! `[batch × seq × vocab]` logits. [`next_token_cross_entropy`] computes the
//! shifted next-token cross-entropy used as the training loss.

use burn::{
    module::Module,
    nn::{
        Embedding, EmbeddingConfig, LayerNorm, LayerNormConfig, Linear, LinearConfig,
        PositionalEncoding, PositionalEncodingConfig,
        attention::generate_autoregressive_mask,
        loss::CrossEntropyLossConfig,
        transformer::{TransformerEncoder, TransformerEncoderConfig, TransformerEncoderInput},
    },
    tensor::{Int, Tensor, backend::Backend},
};

pub use crate::config::ModelConfig;

/// Decoder-only language model — Burn [`Module`] with `forward(tokens) → logits`.
///
/// Built from a [`ModelConfig`] via [`DecoderModel::new`]. Sized by the
/// resolved `vocab_size` on the config (the trained tokenizer's vocabulary, not
/// the [`VOCAB_SIZE`](crate::tokenizer::VOCAB_SIZE) constant).
#[derive(Module, Debug)]
pub struct DecoderModel<B: Backend> {
    token_embedding: Embedding<B>,
    positional_encoding: PositionalEncoding<B>,
    transformer: TransformerEncoder<B>,
    final_norm: LayerNorm<B>,
    lm_head: Linear<B>,
    context_length: usize,
}

impl<B: Backend> DecoderModel<B> {
    /// Instantiate the model from a [`ModelConfig`].
    ///
    /// The transformer stack is built with dropout disabled — dropout is a
    /// training-time hyperparameter that will be wired through the training
    /// config in MULTI-1386, not the architecture config.
    pub fn new(config: &ModelConfig, device: &B::Device) -> Self {
        let token_embedding = EmbeddingConfig::new(config.vocab_size, config.d_model).init(device);
        let positional_encoding = PositionalEncodingConfig::new(config.d_model)
            .with_max_sequence_size(config.context_length)
            .init(device);
        let transformer = TransformerEncoderConfig::new(
            config.d_model,
            config.d_ff,
            config.num_heads,
            config.num_layers,
        )
        .with_norm_first(config.norm_first)
        .with_dropout(0.0)
        .init(device);
        let final_norm = LayerNormConfig::new(config.d_model).init(device);
        // GPT-2 convention: untied LM head with no bias. Untied costs ~12M extra
        // params at vocab=16k, d_model=768 — still in the ~100M class.
        let lm_head = LinearConfig::new(config.d_model, config.vocab_size)
            .with_bias(false)
            .init(device);

        Self {
            token_embedding,
            positional_encoding,
            transformer,
            final_norm,
            lm_head,
            context_length: config.context_length,
        }
    }

    /// Forward pass: tokens `[batch × seq]` → logits `[batch × seq × vocab]`.
    ///
    /// Panics if `seq > context_length` — the positional encoding table is
    /// sized to `context_length` and would otherwise silently truncate.
    pub fn forward(&self, tokens: Tensor<B, 2, Int>) -> Tensor<B, 3> {
        let [batch_size, seq_length] = tokens.dims();
        assert!(
            seq_length <= self.context_length,
            "input sequence length {seq_length} exceeds model context length {}",
            self.context_length,
        );

        let device = tokens.device();
        let hidden = self.token_embedding.forward(tokens);
        let hidden = self.positional_encoding.forward(hidden);

        let mask = generate_autoregressive_mask::<B>(batch_size, seq_length, &device);
        let input = TransformerEncoderInput::new(hidden).mask_attn(mask);
        let hidden = self.transformer.forward(input);

        let hidden = self.final_norm.forward(hidden);
        self.lm_head.forward(hidden)
    }
}

/// Next-token cross-entropy loss with the standard left-shift.
///
/// `logits[:, t, :]` predicts `tokens[:, t + 1]`, so the last logit slot has no
/// target and the first token has no prediction. Flatten the surviving
/// `(batch × (seq − 1))` rows and pass to [`CrossEntropyLoss`].
///
/// At a well-initialized init the returned loss is approximately
/// `ln(vocab_size)` — the entropy of a uniform predictor — and is the canary
/// the acceptance criteria in MULTI-1383 watch.
pub fn next_token_cross_entropy<B: Backend>(
    logits: Tensor<B, 3>,
    tokens: Tensor<B, 2, Int>,
) -> Tensor<B, 1> {
    let [batch_size, seq_length, vocab_size] = logits.dims();
    assert!(
        seq_length >= 2,
        "next-token CE needs at least 2 tokens (one to predict from, one to predict)",
    );
    let pred_len = seq_length - 1;

    let shift_logits = logits
        .slice([0..batch_size, 0..pred_len, 0..vocab_size])
        .reshape([batch_size * pred_len, vocab_size]);
    let shift_targets = tokens
        .slice([0..batch_size, 1..seq_length])
        .reshape([batch_size * pred_len]);

    let device = shift_logits.device();
    CrossEntropyLossConfig::new()
        .init(&device)
        .forward(shift_logits, shift_targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Backend as ActiveBackend, TrainBackend};
    use burn::module::{ModuleVisitor, Param, ParamId};
    use burn::optim::GradientsParams;
    use burn::tensor::ElementConversion;
    use burn::tensor::backend::AutodiffBackend;
    use core::marker::PhantomData;

    fn deterministic_tokens<B: Backend>(
        batch_size: usize,
        seq_length: usize,
        vocab_size: usize,
        device: &B::Device,
    ) -> Tensor<B, 2, Int> {
        // Spread tokens across the vocabulary so the gradient signal touches a
        // diverse slice of embedding rows. The mod keeps every id in range.
        let data: Vec<i64> = (0..(batch_size * seq_length))
            .map(|i| ((i * 7919) % vocab_size) as i64)
            .collect();
        Tensor::<B, 1, Int>::from_data(data.as_slice(), device).reshape([batch_size, seq_length])
    }

    /// The full-size config materializes a model in the ~100M parameter class —
    /// catches a slipped width / depth / vocab against MULTI-1379's locked
    /// vocabulary. Built on the inference backend to keep autodiff bookkeeping
    /// out of the count.
    #[test]
    fn gpt2_small_lands_in_the_100m_param_class() {
        let device = Default::default();
        let model: DecoderModel<ActiveBackend> =
            DecoderModel::new(&ModelConfig::gpt2_small(), &device);
        let params = model.num_params();
        assert!(
            (80_000_000..=140_000_000).contains(&params),
            "expected ~100M parameters, got {params}",
        );
    }

    #[test]
    fn forward_returns_batch_seq_vocab_with_no_nan_or_inf() {
        let device = Default::default();
        let config = ModelConfig::debug_tiny();
        let model: DecoderModel<ActiveBackend> = DecoderModel::new(&config, &device);

        let [batch_size, seq_length] = [2, 16];
        let tokens = deterministic_tokens::<ActiveBackend>(
            batch_size,
            seq_length,
            config.vocab_size,
            &device,
        );

        let logits = model.forward(tokens);
        assert_eq!(logits.dims(), [batch_size, seq_length, config.vocab_size]);

        let data = logits.to_data();
        let values: Vec<f32> = data.to_vec().expect("logits convert to f32");
        assert!(
            values.iter().all(|v| v.is_finite()),
            "logits must be finite (no NaN/Inf) at init",
        );
    }

    /// At init, the next-token CE loss should be close to `ln(vocab_size)` —
    /// the entropy of the uniform predictor a freshly initialized model
    /// approximates. A wide tolerance window keeps this from being flaky
    /// across small init-distribution shifts while still catching gross
    /// init/loss bugs (e.g. a missing log-softmax, wrong vocab dim).
    #[test]
    fn loss_at_init_is_near_ln_vocab() {
        let device = Default::default();
        let config = ModelConfig::debug_tiny();
        let model: DecoderModel<ActiveBackend> = DecoderModel::new(&config, &device);

        let [batch_size, seq_length] = [2, 32];
        let tokens = deterministic_tokens::<ActiveBackend>(
            batch_size,
            seq_length,
            config.vocab_size,
            &device,
        );
        let logits = model.forward(tokens.clone());
        let loss = next_token_cross_entropy(logits, tokens);

        let loss_value: f32 = loss.into_scalar().elem();
        let expected = (config.vocab_size as f32).ln();
        let tolerance = 2.0;
        assert!(
            (loss_value - expected).abs() <= tolerance,
            "expected loss ≈ ln({}) = {expected:.3}, got {loss_value:.3}",
            config.vocab_size,
        );
    }

    /// After a backward pass, every parameter must receive a non-zero
    /// gradient — proves the autograd graph reaches the whole module and no
    /// parameter is accidentally detached.
    #[test]
    fn all_parameters_receive_non_zero_gradients() {
        let device = Default::default();
        let config = ModelConfig::debug_tiny();
        let model: DecoderModel<TrainBackend> = DecoderModel::new(&config, &device);

        let [batch_size, seq_length] = [2, 16];
        let tokens = deterministic_tokens::<TrainBackend>(
            batch_size,
            seq_length,
            config.vocab_size,
            &device,
        );
        let logits = model.forward(tokens.clone());
        let loss = next_token_cross_entropy(logits, tokens);

        let grads = loss.backward();
        let grads = GradientsParams::from_grads(grads, &model);

        let mut checker = GradientChecker::<TrainBackend> {
            grads: &grads,
            visited: 0,
            bad: Vec::new(),
            _phantom: PhantomData,
        };
        model.visit(&mut checker);
        assert!(checker.visited > 0, "model has no parameters to check");
        assert!(
            checker.bad.is_empty(),
            "{} parameter tensor(s) had a missing or all-zero gradient: {:?}",
            checker.bad.len(),
            checker.bad,
        );
    }

    struct GradientChecker<'a, B: AutodiffBackend> {
        grads: &'a GradientsParams,
        visited: usize,
        bad: Vec<ParamId>,
        _phantom: PhantomData<B>,
    }

    impl<B: AutodiffBackend> ModuleVisitor<B> for GradientChecker<'_, B> {
        fn visit_float<const D: usize>(&mut self, param: &Param<Tensor<B, D>>) {
            self.visited += 1;
            let Some(grad) = self.grads.get::<B::InnerBackend, D>(param.id) else {
                self.bad.push(param.id);
                return;
            };
            let abs_sum: f32 = grad.abs().sum().into_scalar().elem();
            if abs_sum == 0.0 {
                self.bad.push(param.id);
            }
        }
    }
}

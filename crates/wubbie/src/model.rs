//! Model definition.
//!
//! The transformer architecture is implemented in a follow-up ticket
//! (MULTI-1383). For now this module re-exports
//! [`ModelConfig`](crate::config::ModelConfig) so downstream code has a stable
//! path to the architecture description.
//!
//! When the model is assembled, size the token embedding table and the LM head
//! from [`ModelConfig::vocab_size`](crate::config::ModelConfig::vocab_size) — the
//! *resolved* config value, not the [`VOCAB_SIZE`](crate::tokenizer::VOCAB_SIZE)
//! constant. That field is derived from the trained `tokenizer.json` (via
//! `wubbie train --tokenizer`), which is the single source of truth for vocab
//! size; the constant is only a default used before a tokenizer exists.

pub use crate::config::ModelConfig;

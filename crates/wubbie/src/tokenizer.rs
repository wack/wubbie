//! Tokenizer loading helpers.
//!
//! wubbie uses the HuggingFace [`tokenizers`] crate. Training the byte-level BPE
//! tokenizer is tracked in MULTI-1379; this module currently loads an already
//! serialized `tokenizer.json`.

use std::path::Path;

use anyhow::Result;
use tokenizers::Tokenizer;

/// Vocabulary size of the locked tokenizer.
///
/// **Placeholder.** The byte-level BPE tokenizer is trained and *locked* in
/// MULTI-1379; until then [`ModelConfig`](crate::config::ModelConfig) is wired
/// to this provisional value so the config system can be scaffolded. When the
/// tokenizer locks, update this single constant (and the README) — every config
/// reads its `vocab_size` from here, so nothing else needs to change.
pub const VOCAB_SIZE: usize = 32_000;

/// Load a serialized tokenizer from a `tokenizer.json` file.
pub fn load(path: impl AsRef<Path>) -> Result<Tokenizer> {
    Tokenizer::from_file(path.as_ref())
        .map_err(|err| anyhow::anyhow!("failed to load tokenizer: {err}"))
}

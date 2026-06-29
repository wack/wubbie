//! Tokenizer loading helpers.
//!
//! wubbie uses the HuggingFace [`tokenizers`] crate. Training the byte-level BPE
//! tokenizer is tracked in MULTI-1379; this module currently loads an already
//! serialized `tokenizer.json`.

use std::path::Path;

use anyhow::Result;
use tokenizers::Tokenizer;

/// Load a serialized tokenizer from a `tokenizer.json` file.
pub fn load(path: impl AsRef<Path>) -> Result<Tokenizer> {
    Tokenizer::from_file(path.as_ref())
        .map_err(|err| anyhow::anyhow!("failed to load tokenizer: {err}"))
}

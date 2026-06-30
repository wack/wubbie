//! The data-loading layer that feeds training (MULTI-1381).
//!
//! This module turns tokenized [WebDataset] tar shards into batches of fixed
//! `[batch × seq]` token windows, deterministically and resumably, scaling from
//! local disk now to streaming later without the trainer changing. It is built
//! as a small stack of single-responsibility pieces:
//!
//! * [`source`] — the [`ShardSource`] trait and its backends: [`LocalDir`]
//!   (built) and [`HfStreaming`] (a deferred not-implemented stub, MULTI-1410).
//! * [`manifest`] — per-shard token/member counts for global ordering.
//! * [`pack`] — concatenate a shard's members and cut fixed `seq_len` windows.
//! * [`split`] — a genuine train/val split at shard granularity (provably no
//!   sequence overlap).
//! * [`rng`] — deterministic, *serializable* RNG primitives (ChaCha8).
//! * [`stream`] — [`WindowStream`], the resumable, shuffled window producer, and
//!   its serializable [`LoaderState`] (the save→kill→resume position).
//! * [`loader`] — [`LmBatch`], the [`ShardLoader`] implementing Burn's
//!   [`DataLoader`](burn::data::dataloader::DataLoader) trait, and the top-level
//!   [`DataPipeline`].
//!
//! The trainer talks to a [`DataPipeline`]: it hands back train/val
//! [`ShardLoader`]s for a `Learner` and the resumable [`WindowStream`] /
//! [`LoaderState`] pair that MULTI-1386/MULTI-1387 persist for exact resume.
//!
//! [WebDataset]: https://github.com/webdataset/webdataset

use serde::{Deserialize, Serialize};

use crate::config::CONTEXT_LENGTH;

pub mod loader;
pub mod manifest;
pub mod pack;
pub mod rng;
pub mod source;
pub mod split;
pub mod stream;

#[cfg(test)]
pub(crate) mod test_support;

pub use loader::{DataPipeline, LmBatch, ShardLoader};
pub use manifest::{Manifest, ShardEntry};
pub use source::{HfStreaming, LocalDir, ShardSource, ShardSourceError};
pub use split::{Split, split_shards};
pub use stream::{LoaderState, WindowStream};

/// Tuning for the data-loading pipeline: how sequences are packed, batched,
/// shuffled, split, and seeded.
///
/// Every randomized choice the loader makes is a deterministic function of
/// [`seed`](Self::seed), so a run reproduces — and resumes — its exact data
/// order from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoaderConfig {
    /// Window length packing targets — the model context length.
    pub seq_len: usize,
    /// Windows per batch (the `batch` in `[batch × seq]`).
    pub batch_size: usize,
    /// Sample shuffle-buffer capacity in windows. `0` disables sample shuffling;
    /// larger values mix windows from further apart in the stream.
    pub shuffle_capacity: usize,
    /// Base seed for the split, shard permutation, and shuffle buffer.
    pub seed: u64,
    /// Fraction of *shards* held out for validation, in `(0, 1)`. The split is at
    /// shard granularity so held-out data never overlaps train (see [`split`]).
    pub val_fraction: f64,
}

impl Default for LoaderConfig {
    fn default() -> Self {
        Self {
            seq_len: CONTEXT_LENGTH,
            batch_size: 32,
            // ~1k windows is a healthy mixing buffer that, at 1024-token windows,
            // costs only a couple of MB of u16 — cheap relative to the model.
            shuffle_capacity: 1_024,
            seed: 0,
            val_fraction: 0.01,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_targets_the_model_context_length() {
        let config = LoaderConfig::default();
        assert_eq!(config.seq_len, CONTEXT_LENGTH);
        assert!(config.val_fraction > 0.0 && config.val_fraction < 1.0);
    }

    #[test]
    fn config_round_trips_through_json() {
        let config = LoaderConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: LoaderConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }
}

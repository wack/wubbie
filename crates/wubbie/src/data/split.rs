//! Genuine train/val split at **shard granularity**.
//!
//! The validation set must be genuinely held out — no sequence may appear in
//! both train and val. The cleanest way to guarantee that is to split whole
//! shards: because packing never crosses a shard boundary
//! (see [`pack`](crate::data::pack)), every window is built from exactly one
//! shard's tokens, so assigning each shard entirely to *either* train or val
//! makes the two window sets provably disjoint. No window-level bookkeeping, no
//! risk of overlap.
//!
//! The assignment is deterministic in the seed: shards are permuted with a
//! seeded RNG and the first `val_fraction` slice is held out, so the same
//! (shards, seed, fraction) always produces the same split.

use anyhow::{Result, ensure};

use super::rng::{seeded, shuffle_in_place};

/// A disjoint partition of shard names into train and validation sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// Shards used for training.
    pub train: Vec<String>,
    /// Shards held out for validation. Disjoint from [`train`](Self::train).
    pub val: Vec<String>,
}

/// A fixed sub-seed that decorrelates the split RNG from the per-epoch shard
/// permutation and the shuffle buffer, all of which derive from the same base
/// seed. Mixed in so "reusing the seed" never accidentally aligns these streams.
const SPLIT_SEED_SALT: u64 = 0x5350_4C49_545F_3031; // "SPLIT_01"

/// Partition `shard_names` into train/val, holding out roughly `val_fraction`
/// of the shards.
///
/// The number held out is `round(val_fraction × n)`, clamped to keep **at least
/// one** shard on each side (so both sets are always non-empty and the
/// held-out guarantee is real). The split is deterministic in `seed`.
///
/// # Errors
///
/// Fails if there are fewer than two shards (cannot hold one out and still
/// train) or if `val_fraction` is not in the open range `(0, 1)`.
pub fn split_shards(shard_names: &[String], val_fraction: f64, seed: u64) -> Result<Split> {
    ensure!(
        shard_names.len() >= 2,
        "need at least 2 shards to hold out a validation set, got {}",
        shard_names.len(),
    );
    ensure!(
        val_fraction > 0.0 && val_fraction < 1.0,
        "val_fraction must be in (0, 1), got {val_fraction}",
    );

    let n = shard_names.len();
    // Round to nearest, then clamp to [1, n-1] so neither side is ever empty.
    let n_val = ((val_fraction * n as f64).round() as usize).clamp(1, n - 1);

    // Permute a copy deterministically, then cut: first `n_val` → val, rest → train.
    let mut permuted = shard_names.to_vec();
    let mut rng = seeded(seed ^ SPLIT_SEED_SALT);
    shuffle_in_place(&mut permuted, &mut rng);
    let train = permuted.split_off(n_val);
    let val = permuted;

    // Restore a stable (sorted) order within each side so downstream shard
    // permutation has a canonical starting point independent of the cut order.
    let mut split = Split { train, val };
    split.train.sort();
    split.val.sort();
    Ok(split)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn shards(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("shard-{i:04}.tar")).collect()
    }

    #[test]
    fn split_is_disjoint_and_covers_every_shard() {
        let all = shards(10);
        let split = split_shards(&all, 0.2, 7).unwrap();

        let train: HashSet<_> = split.train.iter().collect();
        let val: HashSet<_> = split.val.iter().collect();
        assert!(train.is_disjoint(&val), "train and val must not overlap");
        assert_eq!(
            train.len() + val.len(),
            all.len(),
            "every shard assigned exactly once"
        );
        assert_eq!(val.len(), 2, "round(0.2 * 10) held out");
    }

    #[test]
    fn split_is_deterministic_in_the_seed() {
        let all = shards(20);
        assert_eq!(
            split_shards(&all, 0.25, 42).unwrap(),
            split_shards(&all, 0.25, 42).unwrap(),
        );
        assert_ne!(
            split_shards(&all, 0.25, 1).unwrap(),
            split_shards(&all, 0.25, 2).unwrap(),
        );
    }

    #[test]
    fn always_holds_out_at_least_one_shard_each_side() {
        // A tiny fraction over few shards still yields one val shard and one train.
        let split = split_shards(&shards(2), 0.01, 0).unwrap();
        assert_eq!(split.val.len(), 1);
        assert_eq!(split.train.len(), 1);
    }

    #[test]
    fn rejects_too_few_shards_or_bad_fraction() {
        assert!(split_shards(&shards(1), 0.2, 0).is_err());
        assert!(split_shards(&shards(4), 0.0, 0).is_err());
        assert!(split_shards(&shards(4), 1.0, 0).is_err());
    }
}

//! Deterministic, serializable RNG primitives shared by the loader.
//!
//! Every random choice in the data pipeline — the train/val split, the
//! per-epoch shard permutation, and the sample shuffle buffer — runs through
//! these helpers so the whole loader is reproducible from a seed and, crucially,
//! *resumable*: the RNG is [`rand_chacha::ChaCha8Rng`], whose stream position
//! serializes (seed + word position), so a checkpoint can restore the shuffle
//! exactly where it left off.
//!
//! The permutation and bounded-index draws are implemented here directly
//! (Fisher–Yates over explicit `next_u64` draws) rather than via `rand`'s
//! `SliceRandom`, so the produced order is a stable, self-documented function of
//! the RNG stream that won't silently change if a dependency bumps its internal
//! sampling algorithm. That stability is what the "same seed → same order"
//! guarantee rests on.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

/// A `ChaCha8Rng` seeded from a 64-bit value. The single place a seed becomes an
/// RNG, so every stream is constructed the same way.
pub fn seeded(seed: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed)
}

/// A uniformly-distributed index in `[0, n)`.
///
/// Uses rejection sampling over `next_u64` to avoid the modulo bias a plain
/// `next_u64() % n` would introduce, so every index is equally likely.
///
/// # Panics
///
/// Panics if `n == 0` (no index exists).
pub fn bounded(rng: &mut ChaCha8Rng, n: usize) -> usize {
    assert!(n > 0, "bounded index requires n > 0");
    let n = n as u64;
    // Largest multiple of `n` that fits in u64; draws at or above it are rejected
    // so the accepted range is an exact whole number of `n`-sized blocks.
    let limit = (u64::MAX / n) * n;
    loop {
        let x = rng.next_u64();
        if x < limit {
            return (x % n) as usize;
        }
    }
}

/// Shuffle `items` in place with a Fisher–Yates pass driven by `rng`.
pub fn shuffle_in_place<T>(items: &mut [T], rng: &mut ChaCha8Rng) {
    for i in (1..items.len()).rev() {
        let j = bounded(rng, i + 1);
        items.swap(i, j);
    }
}

/// The permutation of `0..n` produced by a Fisher–Yates shuffle under `rng`.
pub fn permutation(n: usize, rng: &mut ChaCha8Rng) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..n).collect();
    shuffle_in_place(&mut indices, rng);
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_stays_in_range_and_covers_it() {
        let mut rng = seeded(123);
        let mut seen = [false; 5];
        for _ in 0..1_000 {
            let i = bounded(&mut rng, 5);
            assert!(i < 5);
            seen[i] = true;
        }
        assert!(seen.iter().all(|&hit| hit), "every index should appear");
    }

    #[test]
    fn bounded_of_one_is_always_zero() {
        let mut rng = seeded(0);
        assert_eq!(bounded(&mut rng, 1), 0);
    }

    #[test]
    fn permutation_is_a_bijection_and_seed_deterministic() {
        let mut a = seeded(7);
        let mut b = seeded(7);
        let pa = permutation(16, &mut a);
        let pb = permutation(16, &mut b);
        assert_eq!(pa, pb, "same seed → same permutation");

        let mut sorted = pa.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            (0..16).collect::<Vec<_>>(),
            "must be a permutation of 0..n"
        );
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = seeded(1);
        let mut b = seeded(2);
        assert_ne!(permutation(32, &mut a), permutation(32, &mut b));
    }

    #[test]
    fn serialized_rng_resumes_the_same_stream() {
        // The property the resumption gate leans on: a serialized ChaCha8Rng
        // continues producing the identical draw sequence after a round-trip.
        let mut rng = seeded(99);
        let _ = rng.next_u64(); // advance off the start
        let snapshot = serde_json::to_string(&rng).unwrap();

        let next_live = rng.next_u64();
        let mut restored: ChaCha8Rng = serde_json::from_str(&snapshot).unwrap();
        assert_eq!(restored.next_u64(), next_live);
    }
}

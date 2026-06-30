//! Sequence packing: turn a shard's variable-length token members into the
//! fixed-length windows the model trains on.
//!
//! A shard's members are tokenized documents of arbitrary length. Training wants
//! dense, fixed-size sequences, so we concatenate a shard's members end-to-end
//! and slice the stream into windows of exactly `seq_len` tokens (the model
//! context length, 1024). The trailing remainder shorter than a full window is
//! dropped.
//!
//! **Packing stays inside a shard** — it never carries leftover tokens across a
//! shard boundary. That keeps each shard's window set a pure function of that
//! shard alone, which is what lets the train/val split (done at shard
//! granularity, see [`split`](crate::data::split)) guarantee *zero* sequence
//! overlap between train and validation: a window is built from exactly one
//! shard's tokens, so disjoint shard sets yield disjoint windows.

/// Pack a shard's `members` into fixed `seq_len`-token windows.
///
/// Members are concatenated in order and cut into windows of exactly `seq_len`;
/// the final partial window (fewer than `seq_len` tokens) is discarded. Returns
/// an empty `Vec` when the shard holds fewer than `seq_len` tokens total.
///
/// # Panics
///
/// Panics if `seq_len == 0` (a zero-length window is meaningless).
pub fn pack_windows<I>(members: I, seq_len: usize) -> Vec<Vec<u16>>
where
    I: IntoIterator<Item = Vec<u16>>,
{
    assert!(seq_len > 0, "seq_len must be non-zero");
    let mut windows = Vec::new();
    let mut current = Vec::with_capacity(seq_len);
    for member in members {
        for token in member {
            current.push(token);
            if current.len() == seq_len {
                windows.push(std::mem::replace(&mut current, Vec::with_capacity(seq_len)));
            }
        }
    }
    // `current` now holds the < seq_len remainder; drop it (no cross-shard carry).
    windows
}

/// The number of `seq_len`-token windows `token_count` tokens pack into.
///
/// This is `token_count / seq_len` — the same count [`pack_windows`] produces —
/// computed from a shard's total token count alone, so the manifest can report
/// per-shard window counts without re-reading the shard.
pub fn window_count(token_count: usize, seq_len: usize) -> usize {
    if seq_len == 0 {
        return 0;
    }
    token_count / seq_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_across_member_boundaries_into_fixed_windows() {
        // 3 + 3 + 2 = 8 tokens, seq_len 4 → two full windows, no remainder.
        let members = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8]];
        assert_eq!(
            pack_windows(members, 4),
            vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8]]
        );
    }

    #[test]
    fn drops_the_trailing_partial_window() {
        // 5 tokens, seq_len 2 → two windows; the lone 5th token is dropped.
        let members = vec![vec![1, 2, 3, 4, 5]];
        assert_eq!(pack_windows(members, 2), vec![vec![1, 2], vec![3, 4]]);
    }

    #[test]
    fn too_few_tokens_yields_no_windows() {
        assert!(pack_windows(vec![vec![1, 2]], 4).is_empty());
    }

    #[test]
    fn window_count_matches_pack_windows_len() {
        let members = vec![vec![1, 2, 3, 4, 5, 6, 7]];
        let total: usize = members.iter().map(Vec::len).sum();
        assert_eq!(window_count(total, 3), pack_windows(members, 3).len());
    }
}

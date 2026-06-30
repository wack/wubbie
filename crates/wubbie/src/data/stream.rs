//! The resumable window stream: the loader's deterministic, restartable heart.
//!
//! A [`WindowStream`] produces the packed token windows of one shard subset
//! (train *or* val) for one epoch, in a reproducible order, with two layers of
//! shuffle:
//!
//! 1. **Shard-order shuffle** — the subset's shards are visited in a per-epoch
//!    permutation seeded from `(seed, epoch)`.
//! 2. **Sample shuffle buffer** — windows stream through a fixed-capacity
//!    reservoir and are emitted in a randomized order, so consecutive windows
//!    from one document don't arrive back-to-back.
//!
//! Both layers are driven by [`ChaCha8Rng`](rand_chacha::ChaCha8Rng), so the
//! order is a pure function of the seed. The whole position — which shard, how
//! far into it, the buffer contents, and the shuffle RNG's stream position —
//! serializes into a [`LoaderState`]. Restoring that state and continuing yields
//! **exactly** the windows the uninterrupted stream would have, which is the
//! save→kill→resume gate the issue requires (and what MULTI-1387's checkpoint
//! persists).
//!
//! Memory stays bounded: only the current shard's windows and the shuffle buffer
//! are held at once — never the whole corpus.

use std::sync::Arc;

use anyhow::Result;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use super::pack::pack_windows;
use super::rng::{bounded, permutation, seeded};
use super::source::ShardSource;

/// Decorrelates the shuffle-buffer RNG from the shard-permutation RNG even
/// though both derive from the same base seed + epoch.
const SHUFFLE_SEED_SALT: u64 = 0x5348_5546_464C_4531; // "SHUFFLE1"

/// Mixes the epoch into a seed so each epoch permutes/shuffles differently while
/// staying a deterministic function of `(seed, epoch)`.
fn mix_epoch(seed: u64, epoch: u64) -> u64 {
    // Multiply the epoch by an odd constant (a 64-bit fractional of the golden
    // ratio) before mixing, so adjacent epochs don't produce correlated streams.
    seed ^ epoch.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// The serializable position of a [`WindowStream`] — everything needed to resume
/// the exact data order after a save/kill.
///
/// Carries the loader position (which shard in the epoch's permutation, and how
/// many windows already drawn from it), the shard permutation's defining inputs
/// (`seed`, `epoch`), and the shuffle buffer's full state (its contents plus the
/// RNG's serialized stream position). MULTI-1387 stores this in the training
/// checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderState {
    /// Base seed shared by the shard permutation and the shuffle buffer.
    pub seed: u64,
    /// The epoch whose shard permutation this position is within.
    pub epoch: u64,
    /// Window length packing targets (the model context length).
    pub seq_len: usize,
    /// Shuffle-buffer capacity in windows (`0` disables sample shuffling).
    pub shuffle_capacity: usize,
    /// Index into the epoch's shard permutation of the shard currently being
    /// read. Equal to the shard count once the producer is drained.
    pub shard_cursor: usize,
    /// How many windows have already been drawn from the current shard — the
    /// count to skip when the shard is re-opened on resume.
    pub windows_drawn_from_shard: usize,
    /// The shuffle buffer's current contents (windows pulled but not yet emitted).
    pub buffer: Vec<Vec<u16>>,
    /// The shuffle RNG, serialized at its exact stream position.
    pub shuffle_rng: ChaCha8Rng,
}

/// A resumable, deterministic stream of packed token windows over one shard
/// subset for one epoch.
///
/// Yields windows via [`try_next`](Self::try_next) (fallible: shard reads can
/// fail) or the infallible [`Iterator`] impl (panics on a read error — corrupt
/// shards are fatal to a run). Snapshot the position with
/// [`snapshot`](Self::snapshot) and resume with [`resume`](Self::resume).
pub struct WindowStream {
    source: Arc<dyn ShardSource>,
    /// The shard subset, in canonical (sorted) order. Indexed by `order`.
    shards: Vec<String>,
    seq_len: usize,
    shuffle_capacity: usize,
    seed: u64,
    epoch: u64,
    /// The epoch's shard permutation: visit `shards[order[shard_cursor]]`.
    order: Vec<usize>,
    shard_cursor: usize,
    windows_drawn_from_shard: usize,
    /// The current shard's remaining windows, lazily loaded.
    current: Option<std::vec::IntoIter<Vec<u16>>>,
    buffer: Vec<Vec<u16>>,
    shuffle_rng: ChaCha8Rng,
}

impl WindowStream {
    /// Start a fresh stream over `shards` for `epoch`.
    ///
    /// `shards` is the train (or val) subset; it is sorted internally so the
    /// per-epoch permutation has a canonical starting point. `shuffle_capacity`
    /// of `0` disables the sample shuffle (windows then arrive in shard-permuted,
    /// packed order).
    pub fn new(
        source: Arc<dyn ShardSource>,
        mut shards: Vec<String>,
        seq_len: usize,
        shuffle_capacity: usize,
        seed: u64,
        epoch: u64,
    ) -> Self {
        assert!(seq_len > 0, "seq_len must be non-zero");
        shards.sort();
        let order = permutation(shards.len(), &mut seeded(mix_epoch(seed, epoch)));
        let shuffle_rng = seeded(mix_epoch(seed ^ SHUFFLE_SEED_SALT, epoch));
        Self {
            source,
            shards,
            seq_len,
            shuffle_capacity,
            seed,
            epoch,
            order,
            shard_cursor: 0,
            windows_drawn_from_shard: 0,
            current: None,
            buffer: Vec::new(),
            shuffle_rng,
        }
    }

    /// Rebuild a stream positioned exactly at a saved [`LoaderState`].
    ///
    /// `shards` must be the same subset the snapshot was taken over (the caller
    /// recomputes the deterministic split). The shard permutation is regenerated
    /// from `(seed, epoch)` and the producer is re-seated at the saved shard and
    /// window offset; the buffer and shuffle RNG are restored verbatim. The next
    /// window this yields is the one the original stream would have yielded next.
    pub fn resume(
        source: Arc<dyn ShardSource>,
        mut shards: Vec<String>,
        state: LoaderState,
    ) -> Self {
        shards.sort();
        let order = permutation(
            shards.len(),
            &mut seeded(mix_epoch(state.seed, state.epoch)),
        );
        Self {
            source,
            shards,
            seq_len: state.seq_len,
            shuffle_capacity: state.shuffle_capacity,
            seed: state.seed,
            epoch: state.epoch,
            order,
            shard_cursor: state.shard_cursor,
            windows_drawn_from_shard: state.windows_drawn_from_shard,
            current: None, // re-opened lazily, skipping `windows_drawn_from_shard`
            buffer: state.buffer,
            shuffle_rng: state.shuffle_rng,
        }
    }

    /// Snapshot the exact current position for later [`resume`](Self::resume).
    pub fn snapshot(&self) -> LoaderState {
        LoaderState {
            seed: self.seed,
            epoch: self.epoch,
            seq_len: self.seq_len,
            shuffle_capacity: self.shuffle_capacity,
            shard_cursor: self.shard_cursor,
            windows_drawn_from_shard: self.windows_drawn_from_shard,
            buffer: self.buffer.clone(),
            shuffle_rng: self.shuffle_rng.clone(),
        }
    }

    /// The epoch this stream is iterating.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Pull the next window straight from the producer (shard-permuted, packed
    /// order), before the shuffle buffer. `Ok(None)` once every shard is drained.
    fn producer_next(&mut self) -> Result<Option<Vec<u16>>> {
        loop {
            if let Some(current) = self.current.as_mut() {
                if let Some(window) = current.next() {
                    self.windows_drawn_from_shard += 1;
                    return Ok(Some(window));
                }
                // Current shard exhausted; advance to the next in the permutation.
                self.current = None;
                self.shard_cursor += 1;
                self.windows_drawn_from_shard = 0;
            }

            if self.shard_cursor >= self.order.len() {
                return Ok(None); // producer drained
            }

            // Open the next (or, on resume, the saved) shard and skip any windows
            // already drawn from it, so a resumed stream picks up mid-shard.
            let name = &self.shards[self.order[self.shard_cursor]];
            let members = self.source.read_shard(name)?;
            let mut windows = pack_windows(members, self.seq_len).into_iter();
            for _ in 0..self.windows_drawn_from_shard {
                windows.next();
            }
            self.current = Some(windows);
        }
    }

    /// Fallibly yield the next window in shuffle order. `Ok(None)` at end of epoch.
    pub fn try_next(&mut self) -> Result<Option<Vec<u16>>> {
        if self.shuffle_capacity == 0 {
            // Shuffle disabled: pass the producer through in order.
            return self.producer_next();
        }

        // Keep the buffer topped up to capacity from the producer.
        while self.buffer.len() < self.shuffle_capacity {
            match self.producer_next()? {
                Some(window) => self.buffer.push(window),
                None => break,
            }
        }
        if self.buffer.is_empty() {
            return Ok(None);
        }

        // Emit a uniformly-random buffered window, then refill its slot so the
        // buffer keeps mixing windows from later in the stream.
        let index = bounded(&mut self.shuffle_rng, self.buffer.len());
        let window = self.buffer.swap_remove(index);
        if let Some(next) = self.producer_next()? {
            self.buffer.push(next);
        }
        Ok(Some(window))
    }
}

impl Iterator for WindowStream {
    type Item = Vec<u16>;

    /// Infallible iteration for the batching path. A shard read error is fatal to
    /// the run (corrupt data can't be silently skipped without desyncing the
    /// deterministic order), so it surfaces as a panic; use
    /// [`try_next`](Self::try_next) where errors must be handled.
    fn next(&mut self) -> Option<Vec<u16>> {
        self.try_next()
            .expect("shard read failed while streaming windows")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::source::LocalDir;
    use crate::data::test_support::{TempDir, write_tar_shard};

    /// A small corpus: each shard's members are distinct token ranges so a window
    /// can be traced back to its shard, and every shard packs into whole windows.
    fn build_corpus(dir: &TempDir, shards: usize, windows_per_shard: usize, seq_len: usize) {
        for s in 0..shards {
            // One member per shard holding `windows_per_shard` full windows.
            let base = (s * windows_per_shard * seq_len) as u16;
            let tokens: Vec<u16> = (0..(windows_per_shard * seq_len) as u16)
                .map(|t| base.wrapping_add(t))
                .collect();
            write_tar_shard(&dir.path().join(format!("shard-{s:02}.tar")), &[tokens]);
        }
    }

    fn shard_names(dir: &TempDir) -> Vec<String> {
        LocalDir::new(dir.path()).shard_names().unwrap()
    }

    fn drain(mut stream: WindowStream) -> Vec<Vec<u16>> {
        let mut out = Vec::new();
        while let Some(w) = stream.try_next().unwrap() {
            out.push(w);
        }
        out
    }

    #[test]
    fn same_seed_same_order_different_seed_differs() {
        let dir = TempDir::new("stream-determinism");
        build_corpus(&dir, 4, 3, 8);
        let source: Arc<dyn ShardSource> = Arc::new(LocalDir::new(dir.path()));
        let names = shard_names(&dir);

        let a = drain(WindowStream::new(
            source.clone(),
            names.clone(),
            8,
            4,
            1234,
            0,
        ));
        let b = drain(WindowStream::new(
            source.clone(),
            names.clone(),
            8,
            4,
            1234,
            0,
        ));
        assert_eq!(a, b, "same seed must reproduce the exact window order");

        let c = drain(WindowStream::new(
            source.clone(),
            names.clone(),
            8,
            4,
            9999,
            0,
        ));
        assert_ne!(a, c, "a different seed should reorder windows");
    }

    #[test]
    fn epoch_changes_the_order_but_not_the_multiset() {
        let dir = TempDir::new("stream-epoch");
        build_corpus(&dir, 4, 3, 8);
        let source: Arc<dyn ShardSource> = Arc::new(LocalDir::new(dir.path()));
        let names = shard_names(&dir);

        let mut e0 = drain(WindowStream::new(source.clone(), names.clone(), 8, 4, 5, 0));
        let mut e1 = drain(WindowStream::new(source.clone(), names.clone(), 8, 4, 5, 1));
        assert_ne!(
            e0, e1,
            "different epochs should visit shards in a different order"
        );

        e0.sort();
        e1.sort();
        assert_eq!(e0, e1, "every epoch must still cover the same windows");
    }

    #[test]
    fn covers_exactly_the_packed_windows_no_overlap() {
        let dir = TempDir::new("stream-coverage");
        build_corpus(&dir, 3, 2, 8); // 3 shards × 2 windows = 6 windows total
        let source: Arc<dyn ShardSource> = Arc::new(LocalDir::new(dir.path()));
        let names = shard_names(&dir);

        let windows = drain(WindowStream::new(source, names, 8, 4, 0, 0));
        assert_eq!(windows.len(), 6);
        // Tokens were globally unique, so no token appears in two windows.
        let mut all: Vec<u16> = windows.into_iter().flatten().collect();
        let count = all.len();
        all.sort();
        all.dedup();
        assert_eq!(
            all.len(),
            count,
            "windows must not share any token (no overlap)"
        );
    }

    #[test]
    fn save_kill_resume_continues_with_no_discontinuity() {
        let dir = TempDir::new("stream-resume");
        build_corpus(&dir, 5, 4, 8); // 5 shards × 4 windows = 20 windows
        let source: Arc<dyn ShardSource> = Arc::new(LocalDir::new(dir.path()));
        let names = shard_names(&dir);

        // The uninterrupted reference order.
        let reference = drain(WindowStream::new(
            source.clone(),
            names.clone(),
            8,
            6,
            77,
            2,
        ));

        // Run partway, snapshot, then drop the stream (the "kill").
        let mut live = WindowStream::new(source.clone(), names.clone(), 8, 6, 77, 2);
        let mut prefix = Vec::new();
        for _ in 0..7 {
            prefix.push(live.try_next().unwrap().unwrap());
        }
        let state = live.snapshot();
        // Round-trip the state through JSON, exactly as a checkpoint would.
        let serialized = serde_json::to_string(&state).unwrap();
        drop(live);

        // Resume from the restored state and drain the rest.
        let restored: LoaderState = serde_json::from_str(&serialized).unwrap();
        let resumed = drain(WindowStream::resume(source, names, restored));

        let mut rejoined = prefix;
        rejoined.extend(resumed);
        assert_eq!(
            rejoined, reference,
            "save→kill→resume must reproduce the uninterrupted order exactly",
        );
    }

    #[test]
    fn resume_at_an_exhausted_producer_drains_remaining_buffer() {
        // Snapshot late, when the producer is drained but the shuffle buffer still
        // holds windows — the resume must emit exactly those, once each.
        let dir = TempDir::new("stream-resume-tail");
        build_corpus(&dir, 2, 2, 8); // 4 windows total
        let source: Arc<dyn ShardSource> = Arc::new(LocalDir::new(dir.path()));
        let names = shard_names(&dir);

        let reference = drain(WindowStream::new(source.clone(), names.clone(), 8, 8, 3, 0));

        let mut live = WindowStream::new(source.clone(), names.clone(), 8, 8, 3, 0);
        let mut prefix = vec![live.try_next().unwrap().unwrap()];
        let state = live.snapshot();
        drop(live);

        let resumed = drain(WindowStream::resume(source, names, state));
        prefix.extend(resumed);
        assert_eq!(prefix, reference);
    }

    #[test]
    fn shuffle_disabled_yields_producer_order() {
        let dir = TempDir::new("stream-noshuffle");
        build_corpus(&dir, 2, 2, 4);
        let source: Arc<dyn ShardSource> = Arc::new(LocalDir::new(dir.path()));
        let names = shard_names(&dir);

        // capacity 0 → deterministic shard-permuted, packed order, no buffer draws.
        let a = drain(WindowStream::new(source.clone(), names.clone(), 4, 0, 1, 0));
        let b = drain(WindowStream::new(source, names, 4, 0, 1, 0));
        assert_eq!(a, b);
        assert_eq!(a.len(), 4);
    }
}

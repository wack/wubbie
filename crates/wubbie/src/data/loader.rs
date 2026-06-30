//! Burn integration: the [`LmBatch`] batch type, the [`ShardLoader`] that
//! implements Burn's [`DataLoader`] trait directly over the streaming pipeline,
//! and the [`DataPipeline`] that wires a [`ShardSource`] + manifest + split into
//! ready-to-train train/val loaders.
//!
//! The loader implements Burn's [`DataLoader`] **trait** (`iter` / `num_items` /
//! `to_device` / `slice`) rather than the map-style `Dataset` trait, because the
//! corpus is a stream packed and shuffled on the fly, not a randomly-indexable
//! collection. A `Learner` consumes a `ShardLoader` exactly as it would Burn's
//! own batch loader.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use burn::data::dataloader::{DataLoader, DataLoaderIterator, Progress};
use burn::tensor::{Int, Tensor, backend::Backend};

use super::LoaderConfig;
use super::manifest::Manifest;
use super::source::ShardSource;
use super::split::{Split, split_shards};
use super::stream::{LoaderState, WindowStream};

/// One training batch: a `[batch × seq]` tensor of token ids.
///
/// The shape the DoD calls for. The model's `forward` takes exactly this tensor
/// and computes the shifted next-token loss from it, so a `ShardLoader` feeds a
/// `Learner` with no adapter in between.
#[derive(Debug, Clone)]
pub struct LmBatch<B: Backend> {
    /// Token ids, shape `[batch_size, seq_len]`.
    pub tokens: Tensor<B, 2, Int>,
}

/// Collate `windows` (each exactly `seq_len` tokens) into a `[batch × seq]` batch
/// on `device`. `uint16` ids widen to `i64`, the element type Burn's `Int`
/// tensors carry.
fn collate<B: Backend>(windows: &[Vec<u16>], device: &B::Device) -> LmBatch<B> {
    let batch = windows.len();
    let seq = windows[0].len();
    let data: Vec<i64> = windows
        .iter()
        .flat_map(|window| window.iter().map(|&token| token as i64))
        .collect();
    let tokens = Tensor::<B, 1, Int>::from_data(data.as_slice(), device).reshape([batch, seq]);
    LmBatch { tokens }
}

/// A Burn data loader over the streaming, shuffled, packed shard pipeline.
///
/// Each [`iter`](DataLoader::iter) makes one epoch's pass over its shard subset,
/// advancing an internal epoch counter so successive epochs visit shards in a
/// fresh (but seed-deterministic) order — mirroring how Burn's own loader
/// advances its shuffle RNG per iteration. Resumable mid-epoch training runs go
/// through [`WindowStream`]/[`LoaderState`] (see [`DataPipeline`]); this trait
/// impl is the `Learner`-facing surface.
pub struct ShardLoader<B: Backend> {
    source: Arc<dyn ShardSource>,
    /// The shard subset (train or val) this loader draws from.
    shards: Vec<String>,
    seq_len: usize,
    batch_size: usize,
    shuffle_capacity: usize,
    seed: u64,
    /// Total `seq_len`-windows in the subset, from the manifest — `num_items`.
    total_windows: usize,
    device: B::Device,
    /// Shared, interior-mutable epoch counter; each `iter()` consumes one epoch.
    epoch: Arc<Mutex<u64>>,
    /// Optional `[start, end)` window-index restriction set by [`slice`](DataLoader::slice).
    range: Option<(usize, usize)>,
}

impl<B: Backend> ShardLoader<B> {
    /// The window range this loader will actually emit, as `(start, end)` indices.
    fn effective_range(&self) -> (usize, usize) {
        self.range.unwrap_or((0, self.total_windows))
    }

    /// Clone the loader's configuration, overriding the device and range.
    fn rebuild(
        &self,
        device: B::Device,
        range: Option<(usize, usize)>,
        epoch: Arc<Mutex<u64>>,
    ) -> Self {
        Self {
            source: self.source.clone(),
            shards: self.shards.clone(),
            seq_len: self.seq_len,
            batch_size: self.batch_size,
            shuffle_capacity: self.shuffle_capacity,
            seed: self.seed,
            total_windows: self.total_windows,
            device,
            epoch,
            range,
        }
    }
}

impl<B: Backend> DataLoader<B, LmBatch<B>> for ShardLoader<B> {
    fn iter<'a>(&'a self) -> Box<dyn DataLoaderIterator<LmBatch<B>> + 'a> {
        // Consume one epoch from the shared counter, so each pass reshuffles.
        let epoch = {
            let mut guard = self.epoch.lock().expect("epoch counter poisoned");
            let epoch = *guard;
            *guard = guard.wrapping_add(1);
            epoch
        };
        let stream = WindowStream::new(
            self.source.clone(),
            self.shards.clone(),
            self.seq_len,
            self.shuffle_capacity,
            self.seed,
            epoch,
        );
        let (start, end) = self.effective_range();
        Box::new(ShardLoaderIterator {
            stream,
            batch_size: self.batch_size,
            device: self.device.clone(),
            skip_remaining: start,
            emit_remaining: end.saturating_sub(start),
            items_total: end.saturating_sub(start),
            items_processed: 0,
        })
    }

    fn num_items(&self) -> usize {
        let (start, end) = self.effective_range();
        end.saturating_sub(start)
    }

    fn to_device(&self, device: &B::Device) -> Arc<dyn DataLoader<B, LmBatch<B>>> {
        // Share the epoch counter so the moved-to-device loader keeps advancing
        // the same epoch sequence.
        Arc::new(self.rebuild(device.clone(), self.range, self.epoch.clone()))
    }

    fn slice(&self, start: usize, end: usize) -> Arc<dyn DataLoader<B, LmBatch<B>>> {
        // Restrict to the [start, end) window range. Fork the epoch counter to the
        // current epoch so the slice sees the same shuffle as its parent would.
        let (base_start, base_end) = self.effective_range();
        let restricted = (
            base_start + start.min(base_end - base_start),
            base_start + end.min(base_end - base_start),
        );
        let forked = Arc::new(Mutex::new(
            *self.epoch.lock().expect("epoch counter poisoned"),
        ));
        Arc::new(self.rebuild(self.device.clone(), Some(restricted), forked))
    }
}

/// Iterator over one epoch's batches, applying the slice range and dropping a
/// final partial batch so every yielded batch is exactly `[batch_size × seq_len]`.
struct ShardLoaderIterator<B: Backend> {
    stream: WindowStream,
    batch_size: usize,
    device: B::Device,
    /// Windows still to skip before the slice's start.
    skip_remaining: usize,
    /// Windows still allowed to emit within the slice.
    emit_remaining: usize,
    items_total: usize,
    items_processed: usize,
}

impl<B: Backend> Iterator for ShardLoaderIterator<B> {
    type Item = LmBatch<B>;

    fn next(&mut self) -> Option<LmBatch<B>> {
        // Honor the slice start by discarding leading windows.
        while self.skip_remaining > 0 {
            self.stream.next()?;
            self.skip_remaining -= 1;
        }
        if self.emit_remaining < self.batch_size {
            return None; // can't form a full batch within the slice — drop the tail
        }

        let mut windows = Vec::with_capacity(self.batch_size);
        for _ in 0..self.batch_size {
            match self.stream.next() {
                Some(window) => windows.push(window),
                None => return None, // producer drained mid-batch → drop partial batch
            }
        }
        self.emit_remaining -= self.batch_size;
        self.items_processed += self.batch_size;
        Some(collate(&windows, &self.device))
    }
}

impl<B: Backend> DataLoaderIterator<LmBatch<B>> for ShardLoaderIterator<B> {
    fn progress(&self) -> Progress {
        Progress::new(self.items_processed, self.items_total)
    }
}

/// The top-level data layer: a [`ShardSource`] plus its manifest and a
/// deterministic train/val split, minting the train/val loaders and the
/// resumable streams the trainer drives.
pub struct DataPipeline {
    source: Arc<dyn ShardSource>,
    manifest: Manifest,
    split: Split,
    config: LoaderConfig,
}

impl DataPipeline {
    /// Build a pipeline, deriving the manifest by scanning the source once.
    ///
    /// Use [`with_manifest`](Self::with_manifest) when MULTI-1380 shipped a
    /// manifest to avoid the scan.
    pub fn new(source: Arc<dyn ShardSource>, config: LoaderConfig) -> Result<Self> {
        let manifest = Manifest::build(source.as_ref())?;
        Self::with_manifest(source, manifest, config)
    }

    /// Build a pipeline from an already-known manifest (the production path).
    pub fn with_manifest(
        source: Arc<dyn ShardSource>,
        manifest: Manifest,
        config: LoaderConfig,
    ) -> Result<Self> {
        manifest.validate()?;
        let names: Vec<String> = manifest.shards.iter().map(|e| e.name.clone()).collect();
        let split = split_shards(&names, config.val_fraction, config.seed)?;
        Ok(Self {
            source,
            manifest,
            split,
            config,
        })
    }

    /// The train/val shard split.
    pub fn split(&self) -> &Split {
        &self.split
    }

    /// The manifest backing this pipeline.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Total training windows (across the train shards) at the configured `seq_len`.
    pub fn train_windows(&self) -> usize {
        self.manifest
            .windows_in(&self.split.train, self.config.seq_len)
    }

    /// Total validation windows (across the held-out shards).
    pub fn val_windows(&self) -> usize {
        self.manifest
            .windows_in(&self.split.val, self.config.seq_len)
    }

    /// A fresh resumable training stream for `epoch`.
    pub fn train_stream(&self, epoch: u64) -> WindowStream {
        WindowStream::new(
            self.source.clone(),
            self.split.train.clone(),
            self.config.seq_len,
            self.config.shuffle_capacity,
            self.config.seed,
            epoch,
        )
    }

    /// A validation stream for `epoch`. Validation needs no sample shuffle, so its
    /// buffer is disabled — the order is a stable pass over the held-out shards.
    pub fn val_stream(&self, epoch: u64) -> WindowStream {
        WindowStream::new(
            self.source.clone(),
            self.split.val.clone(),
            self.config.seq_len,
            0,
            self.config.seed,
            epoch,
        )
    }

    /// Resume a training stream from a persisted [`LoaderState`] (the resume gate).
    pub fn resume_train(&self, state: LoaderState) -> WindowStream {
        WindowStream::resume(self.source.clone(), self.split.train.clone(), state)
    }

    /// A Burn [`DataLoader`] over the training shards, batches on `device`.
    pub fn train_loader<B: Backend>(&self, device: B::Device) -> ShardLoader<B> {
        self.shard_loader(
            self.split.train.clone(),
            self.train_windows(),
            self.config.shuffle_capacity,
            device,
        )
    }

    /// A Burn [`DataLoader`] over the held-out validation shards.
    pub fn val_loader<B: Backend>(&self, device: B::Device) -> ShardLoader<B> {
        self.shard_loader(self.split.val.clone(), self.val_windows(), 0, device)
    }

    fn shard_loader<B: Backend>(
        &self,
        shards: Vec<String>,
        total_windows: usize,
        shuffle_capacity: usize,
        device: B::Device,
    ) -> ShardLoader<B> {
        ShardLoader {
            source: self.source.clone(),
            shards,
            seq_len: self.config.seq_len,
            batch_size: self.config.batch_size,
            shuffle_capacity,
            seed: self.config.seed,
            total_windows,
            device,
            epoch: Arc::new(Mutex::new(0)),
            range: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Backend as ActiveBackend;
    use crate::data::source::LocalDir;
    use crate::data::test_support::{TempDir, write_tar_shard};

    /// Build `shards` shards each packing `windows_per_shard` full `seq_len`
    /// windows of globally-unique token ids.
    fn build_corpus(dir: &TempDir, shards: usize, windows_per_shard: usize, seq_len: usize) {
        for s in 0..shards {
            let base = (s * windows_per_shard * seq_len) as u16;
            let tokens: Vec<u16> = (0..(windows_per_shard * seq_len) as u16)
                .map(|t| base.wrapping_add(t))
                .collect();
            write_tar_shard(&dir.path().join(format!("shard-{s:02}.tar")), &[tokens]);
        }
    }

    fn pipeline(dir: &TempDir, config: LoaderConfig) -> DataPipeline {
        let source: Arc<dyn ShardSource> = Arc::new(LocalDir::new(dir.path()));
        DataPipeline::new(source, config).unwrap()
    }

    fn small_config(seq_len: usize, batch_size: usize) -> LoaderConfig {
        LoaderConfig {
            seq_len,
            batch_size,
            shuffle_capacity: 4,
            seed: 17,
            val_fraction: 0.25,
        }
    }

    #[test]
    fn pulled_batch_has_shape_batch_by_seq() {
        let dir = TempDir::new("loader-shape");
        build_corpus(&dir, 8, 3, 8);
        let pipeline = pipeline(&dir, small_config(8, 4));

        let loader = pipeline.train_loader::<ActiveBackend>(Default::default());
        let batch = loader.iter().next().expect("at least one batch");
        assert_eq!(
            batch.tokens.dims(),
            [4, 8],
            "batch must be [batch_size, seq_len]"
        );
    }

    #[test]
    fn validation_is_genuinely_held_out_no_shard_overlap() {
        let dir = TempDir::new("loader-split");
        build_corpus(&dir, 8, 3, 8);
        let pipeline = pipeline(&dir, small_config(8, 4));

        let split = pipeline.split();
        assert!(!split.val.is_empty(), "a validation set must exist");
        for val_shard in &split.val {
            assert!(
                !split.train.contains(val_shard),
                "val shard {val_shard} must not appear in train — no overlap",
            );
        }
        // Window accounting: train + val windows == every packed window.
        assert_eq!(
            pipeline.train_windows() + pipeline.val_windows(),
            pipeline.manifest().total_windows(8),
        );
    }

    #[test]
    fn train_and_val_windows_never_share_a_token() {
        // The strongest form of "genuinely held out": collect every train token
        // and every val token and prove the sets are disjoint.
        use std::collections::HashSet;
        let dir = TempDir::new("loader-noverlap");
        build_corpus(&dir, 6, 2, 8);
        let pipeline = pipeline(&dir, small_config(8, 2));

        let collect = |mut stream: WindowStream| -> HashSet<u16> {
            let mut set = HashSet::new();
            while let Some(window) = stream.try_next().unwrap() {
                set.extend(window);
            }
            set
        };
        let train_tokens = collect(pipeline.train_stream(0));
        let val_tokens = collect(pipeline.val_stream(0));
        assert!(
            train_tokens.is_disjoint(&val_tokens),
            "no token may appear in both train and validation windows",
        );
    }

    #[test]
    fn num_items_matches_the_manifest_window_count() {
        let dir = TempDir::new("loader-numitems");
        build_corpus(&dir, 8, 3, 8);
        let pipeline = pipeline(&dir, small_config(8, 4));
        let loader = pipeline.train_loader::<ActiveBackend>(Default::default());
        assert_eq!(loader.num_items(), pipeline.train_windows());
    }

    #[test]
    fn iter_is_seed_deterministic() {
        let dir = TempDir::new("loader-determinism");
        build_corpus(&dir, 8, 3, 8);

        let batches = |pipeline: &DataPipeline| -> Vec<Vec<i64>> {
            let loader = pipeline.train_loader::<ActiveBackend>(Default::default());
            loader
                .iter()
                .map(|batch| batch.tokens.to_data().to_vec::<i64>().unwrap())
                .collect()
        };

        // Two independent pipelines with the same seed must produce identical
        // first-epoch batches.
        let a = batches(&pipeline(&dir, small_config(8, 4)));
        let b = batches(&pipeline(&dir, small_config(8, 4)));
        assert_eq!(a, b, "same seed → same batch order and contents");
    }

    #[test]
    fn loader_does_not_load_full_corpus_smoke() {
        // A coarse guard that iteration works end-to-end over many shards without
        // materializing everything: drain the loader and count batches.
        let dir = TempDir::new("loader-stream");
        build_corpus(&dir, 10, 4, 8);
        let pipeline = pipeline(&dir, small_config(8, 4));
        let loader = pipeline.train_loader::<ActiveBackend>(Default::default());
        let batches = loader.iter().count();
        assert!(batches > 0);
        // Every emitted batch is full (partial tail dropped), so batches*batch_size
        // never exceeds the available training windows.
        assert!(batches * 4 <= pipeline.train_windows() + 4);
    }
}

//! Test-only helpers shared across the `data` module's unit tests: a
//! self-cleaning temp directory and a writer that synthesizes WebDataset tar
//! shards of `uint16` token windows (the format [`LocalDir`](super::LocalDir)
//! reads). Compiled only under `cfg(test)`.

use std::fs::File;
use std::path::{Path, PathBuf};

/// A temp directory that removes itself (and its contents) on drop, so tests
/// leave nothing behind even when they fail. Named per-test + per-process so
/// concurrent `cargo nextest` runs never collide on a shared path.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create a uniquely-named temp directory tagged with `label`.
    pub fn new(label: &str) -> Self {
        // A monotonically increasing counter keeps two `TempDir`s made in the
        // same test (same pid) on distinct paths without reaching for a clock or
        // `thread_rng` (both barred by the no-flake testing policy).
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("wubbie-data-{label}-{}-{n}", std::process::id(),));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    /// The directory path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Raw little-endian `uint16` bytes for `tokens` — the on-disk member encoding.
pub fn token_bytes(tokens: &[u16]) -> Vec<u8> {
    tokens.iter().flat_map(|t| t.to_le_bytes()).collect()
}

/// Write a WebDataset tar shard at `path`, one `NNNNNN.tokens` member per
/// `members` entry, each holding that slice's tokens as little-endian `uint16`.
pub fn write_tar_shard(path: &Path, members: &[Vec<u16>]) {
    let file = File::create(path).expect("create shard file");
    let mut builder = tar::Builder::new(file);
    for (index, tokens) in members.iter().enumerate() {
        let bytes = token_bytes(tokens);
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, format!("{index:06}.tokens"), bytes.as_slice())
            .expect("append tar member");
    }
    builder.finish().expect("finish tar shard");
}

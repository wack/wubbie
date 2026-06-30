//! The [`ShardSource`] trait and its backends.
//!
//! A `ShardSource` abstracts *where* the tokenized [WebDataset] tar shards live,
//! so the rest of the loader — packing, shuffle, batching, split, resume — is
//! written once and works unchanged whether the bytes come from local disk now
//! or a streaming store later. Two backends implement it:
//!
//! * [`LocalDir`] — **the v1 backend, fully built.** Reads `*.tar` shards from a
//!   local directory (the corpus hydrated onto local NVMe). Each tar member is a
//!   `<key>.tokens` file holding one document's tokens as raw little-endian
//!   `uint16` (the on-disk dtype locked by MULTI-1380; `uint16` fits the ≤64k
//!   vocabulary). Shards are read **one at a time**, so the full corpus is never
//!   held in memory.
//! * [`HfStreaming`] — **a deferred stub.** Present so the trait surface exists
//!   and compiles; every method returns [`ShardSourceError::NotImplemented`].
//!   The real async streaming implementation is tracked in MULTI-1410.
//!
//! [WebDataset]: https://github.com/webdataset/webdataset

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The tar-member extension that marks a token window. Members with any other
/// extension (e.g. a `.json` sidecar) are skipped, so a shard can carry metadata
/// alongside its token files without confusing the loader.
pub const TOKEN_MEMBER_EXT: &str = "tokens";

/// Errors a [`ShardSource`] can surface that are worth matching on by type.
#[derive(Debug, thiserror::Error)]
pub enum ShardSourceError {
    /// The [`HfStreaming`] backend is not implemented yet (tracked in
    /// MULTI-1410). Returned by every `HfStreaming` method so the deferred
    /// backend fails with a typed, explicit error rather than a panic.
    #[error(
        "the HfStreaming ShardSource backend is not implemented yet \
         (deferred to MULTI-1410); use the LocalDir backend"
    )]
    NotImplemented,
}

/// A source of tokenized WebDataset tar shards.
///
/// Implementors expose the set of shard names and, on demand, the token windows
/// of a single shard. Reading is deliberately **shard-at-a-time**: the loader
/// pulls one shard's members, packs and consumes them, and only then moves to
/// the next shard, so memory stays bounded by a single shard rather than the
/// whole corpus.
pub trait ShardSource: Send + Sync {
    /// The names of every shard this source offers, in a stable, sorted order.
    ///
    /// The order must be deterministic across calls and processes: the
    /// train/val split and the per-epoch shard permutation are both derived
    /// from it, so a reshuffled listing would silently change which data is held
    /// out and in what order it is read.
    fn shard_names(&self) -> Result<Vec<String>>;

    /// Read every token member of one shard, in tar order.
    ///
    /// Returns one `Vec<u16>` per `*.tokens` member (a document's token ids);
    /// non-token members are skipped. The caller packs these into fixed-length
    /// windows (see [`pack`](crate::data::pack)).
    fn read_shard(&self, name: &str) -> Result<Vec<Vec<u16>>>;
}

/// Reads WebDataset tar shards from a local directory. The v1 backend.
#[derive(Debug, Clone)]
pub struct LocalDir {
    root: PathBuf,
}

impl LocalDir {
    /// A source reading `*.tar` shards from `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory shards are read from.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Whether `name` is a tar shard we should read.
fn is_tar_shard(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".tar")
}

/// Whether a tar member `name` is a token window (`*.tokens`).
fn is_token_member(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(TOKEN_MEMBER_EXT))
}

/// Decode a little-endian `uint16` byte blob into token ids. A trailing odd byte
/// (a truncated final token) is dropped via [`slice::chunks_exact`].
fn decode_u16_le(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

impl ShardSource for LocalDir {
    fn shard_names(&self) -> Result<Vec<String>> {
        let mut names: Vec<String> = std::fs::read_dir(&self.root)
            .with_context(|| format!("cannot read shard directory: {}", self.root.display()))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                (entry.path().is_file() && is_tar_shard(&name)).then_some(name)
            })
            .collect();
        names.sort();
        Ok(names)
    }

    fn read_shard(&self, name: &str) -> Result<Vec<Vec<u16>>> {
        let path = self.root.join(name);
        let file =
            File::open(&path).with_context(|| format!("cannot open shard: {}", path.display()))?;
        let mut archive = tar::Archive::new(BufReader::new(file));
        let mut windows = Vec::new();
        for entry in archive
            .entries()
            .with_context(|| format!("cannot read shard archive: {}", path.display()))?
        {
            let mut entry =
                entry.with_context(|| format!("corrupt entry in shard: {}", path.display()))?;
            let member = entry
                .path()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !is_token_member(&member) {
                continue;
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut bytes)
                .with_context(|| format!("cannot read member {member} in shard: {name}"))?;
            windows.push(decode_u16_le(&bytes));
        }
        Ok(windows)
    }
}

/// Streaming WebDataset shard source — **deferred stub** (MULTI-1410).
///
/// Constructing one is fine (so callers and config can reference it), but every
/// trait method returns [`ShardSourceError::NotImplemented`]. The real backend
/// will stream shards from the remote store with async prefetch + a local cache;
/// none of that exists yet, and it is intentionally out of scope here.
#[derive(Debug, Clone, Default)]
pub struct HfStreaming {
    /// The dataset/repo the shards would stream from. Stored so the field shape
    /// is right; unused until the backend is implemented.
    pub repo: String,
}

impl HfStreaming {
    /// A stub source for `repo`. See the type docs: every read returns
    /// [`ShardSourceError::NotImplemented`].
    pub fn new(repo: impl Into<String>) -> Self {
        Self { repo: repo.into() }
    }
}

impl ShardSource for HfStreaming {
    fn shard_names(&self) -> Result<Vec<String>> {
        Err(ShardSourceError::NotImplemented.into())
    }

    fn read_shard(&self, _name: &str) -> Result<Vec<Vec<u16>>> {
        Err(ShardSourceError::NotImplemented.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_support::{TempDir, token_bytes, write_tar_shard};

    #[test]
    fn decode_u16_le_drops_a_trailing_odd_byte() {
        // 0x0001 LE, 0x0302 LE, then a dangling 0xFF that can't form a token.
        let bytes = [0x01, 0x00, 0x02, 0x03, 0xFF];
        assert_eq!(decode_u16_le(&bytes), vec![0x0001, 0x0302]);
    }

    #[test]
    fn local_dir_lists_only_tar_shards_sorted() {
        let dir = TempDir::new("source-list");
        write_tar_shard(&dir.path().join("b.tar"), &[vec![1, 2]]);
        write_tar_shard(&dir.path().join("a.tar"), &[vec![3, 4]]);
        std::fs::write(dir.path().join("notes.txt"), b"ignore me").unwrap();

        let source = LocalDir::new(dir.path());
        assert_eq!(source.shard_names().unwrap(), ["a.tar", "b.tar"]);
    }

    #[test]
    fn local_dir_reads_token_members_and_skips_sidecars() {
        let dir = TempDir::new("source-read");
        let path = dir.path().join("shard.tar");
        // Two token members plus a `.json` sidecar that must be ignored.
        write_tar_shard_mixed(
            &path,
            &[
                ("000000.tokens", token_bytes(&[1, 2, 3])),
                ("000000.json", b"{\"meta\":1}".to_vec()),
                ("000001.tokens", token_bytes(&[4, 5])),
            ],
        );

        let source = LocalDir::new(dir.path());
        let members = source.read_shard("shard.tar").unwrap();
        assert_eq!(members, vec![vec![1, 2, 3], vec![4, 5]]);
    }

    #[test]
    fn hf_streaming_is_an_explicit_not_implemented_stub() {
        let source = HfStreaming::new("owner/shards");
        let err = source.shard_names().unwrap_err();
        assert!(err.downcast_ref::<ShardSourceError>().is_some());
        assert!(source.read_shard("000000.tar").is_err());
    }

    /// Write a tar with explicitly-named members (token files + sidecars).
    fn write_tar_shard_mixed(path: &Path, members: &[(&str, Vec<u8>)]) {
        let file = File::create(path).unwrap();
        let mut builder = tar::Builder::new(file);
        for (name, bytes) in members {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, bytes.as_slice())
                .unwrap();
        }
        builder.finish().unwrap();
    }
}

//! The shard/sample **manifest**: per-shard token and member counts that give
//! the loader global ordering without reading the corpus.
//!
//! MULTI-1380 emits a manifest alongside the tar shards so the loader knows each
//! shard's size up front — how many [`window_count`](crate::data::pack::window_count)
//! windows it yields at a given `seq_len`, and the total across the run. The
//! loader consumes that manifest when present and, for local runs or tests where
//! one wasn't shipped, can [`build`](Manifest::build) it by scanning the source
//! once. Either way the rest of the loader treats it as the authority on shard
//! sizes.

use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::pack::window_count;
use super::source::ShardSource;

/// One shard's entry in the [`Manifest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardEntry {
    /// The shard's name within its source (e.g. `shard-00042.tar`).
    pub name: String,
    /// Number of token *members* (documents) in the shard.
    pub members: usize,
    /// Total `uint16` tokens across all of the shard's members.
    pub tokens: usize,
}

impl ShardEntry {
    /// How many `seq_len`-token windows this shard packs into.
    pub fn windows(&self, seq_len: usize) -> usize {
        window_count(self.tokens, seq_len)
    }
}

/// Per-shard token/member counts for a sharded corpus.
///
/// Serializes to JSON as `{ "shards": [ { name, members, tokens }, … ] }`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Every shard, in the source's stable order.
    pub shards: Vec<ShardEntry>,
}

impl Manifest {
    /// Build a manifest by scanning every shard in `source` once.
    ///
    /// This reads the corpus shard-by-shard to count tokens; it is the fallback
    /// for sources that didn't ship a manifest. Production runs load the
    /// MULTI-1380 manifest with [`load`](Self::load) instead of rebuilding it.
    pub fn build(source: &dyn ShardSource) -> Result<Self> {
        let names = source.shard_names()?;
        let mut shards = Vec::with_capacity(names.len());
        for name in names {
            let members = source.read_shard(&name)?;
            let tokens = members.iter().map(Vec::len).sum();
            shards.push(ShardEntry {
                name,
                members: members.len(),
                tokens,
            });
        }
        Ok(Self { shards })
    }

    /// Load a manifest from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .with_context(|| format!("cannot read manifest: {}", path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("cannot parse manifest: {}", path.display()))
    }

    /// Write the manifest to a JSON file (pretty-printed).
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let json = serde_json::to_string_pretty(self).context("cannot serialize manifest")?;
        std::fs::write(path, json)
            .with_context(|| format!("cannot write manifest: {}", path.display()))
    }

    /// The shard entry named `name`, if present.
    pub fn shard(&self, name: &str) -> Option<&ShardEntry> {
        self.shards.iter().find(|entry| entry.name == name)
    }

    /// Total tokens across every shard.
    pub fn total_tokens(&self) -> usize {
        self.shards.iter().map(|entry| entry.tokens).sum()
    }

    /// Total `seq_len`-token windows across every shard.
    pub fn total_windows(&self, seq_len: usize) -> usize {
        self.shards.iter().map(|entry| entry.windows(seq_len)).sum()
    }

    /// Total windows across just the named shards (e.g. the train or val subset).
    ///
    /// Names not present in the manifest contribute zero.
    pub fn windows_in(&self, names: &[String], seq_len: usize) -> usize {
        names
            .iter()
            .filter_map(|name| self.shard(name))
            .map(|entry| entry.windows(seq_len))
            .sum()
    }

    /// Check the manifest is internally consistent: shard names are unique and
    /// non-empty. (Per-shard counts are trusted as recorded.)
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.shards.is_empty(), "manifest lists no shards");
        let mut seen = std::collections::HashSet::new();
        for entry in &self.shards {
            ensure!(
                !entry.name.is_empty(),
                "manifest has a shard with an empty name"
            );
            ensure!(
                seen.insert(entry.name.as_str()),
                "manifest lists shard {:?} more than once",
                entry.name,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_support::{TempDir, write_tar_shard};

    #[test]
    fn build_counts_members_and_tokens_per_shard() {
        let dir = TempDir::new("manifest-build");
        write_tar_shard(&dir.path().join("a.tar"), &[vec![1, 2, 3], vec![4, 5]]);
        write_tar_shard(&dir.path().join("b.tar"), &[vec![6, 7, 8, 9]]);

        let source = crate::data::source::LocalDir::new(dir.path());
        let manifest = Manifest::build(&source).unwrap();

        assert_eq!(manifest.shards.len(), 2);
        assert_eq!(
            manifest.shard("a.tar").unwrap(),
            &ShardEntry {
                name: "a.tar".to_owned(),
                members: 2,
                tokens: 5,
            }
        );
        assert_eq!(manifest.shard("b.tar").unwrap().tokens, 4);
        assert_eq!(manifest.total_tokens(), 9);
    }

    #[test]
    fn window_counts_floor_per_shard_then_sum() {
        let manifest = Manifest {
            shards: vec![
                ShardEntry {
                    name: "a.tar".into(),
                    members: 1,
                    tokens: 7,
                },
                ShardEntry {
                    name: "b.tar".into(),
                    members: 1,
                    tokens: 5,
                },
            ],
        };
        // floor(7/3)=2, floor(5/3)=1 → 3 total. Per-shard flooring (not flooring
        // the global sum) is what keeps windows shard-local.
        assert_eq!(manifest.total_windows(3), 3);
        assert_eq!(manifest.windows_in(&["a.tar".to_owned()], 3), 2);
    }

    #[test]
    fn round_trips_through_json() {
        let dir = TempDir::new("manifest-json");
        let manifest = Manifest {
            shards: vec![ShardEntry {
                name: "a.tar".into(),
                members: 3,
                tokens: 100,
            }],
        };
        let path = dir.path().join("manifest.json");
        manifest.save(&path).unwrap();
        assert_eq!(Manifest::load(&path).unwrap(), manifest);
    }

    #[test]
    fn validate_rejects_empty_and_duplicate_shards() {
        assert!(Manifest::default().validate().is_err());
        let dup = Manifest {
            shards: vec![
                ShardEntry {
                    name: "a.tar".into(),
                    members: 1,
                    tokens: 1,
                },
                ShardEntry {
                    name: "a.tar".into(),
                    members: 1,
                    tokens: 1,
                },
            ],
        };
        assert!(dup.validate().is_err());
    }
}
